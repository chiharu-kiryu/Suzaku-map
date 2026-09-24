// Development-only observer for an owned VS Code instance. This extension never
// edits a document or injects text: only XTest -> Electron -> IBus supplies input.
'use strict';
const vscode = require('vscode');
const http = require('node:http');
const path = require('node:path');
const fs = require('node:fs');

exports.activate = async function (context) {
  const root = process.env.SUZAKU_APP_QA_ROOT || '';
  const base = process.env.SUZAKU_CODE_QA_BASE || '';
  const token = process.env.SUZAKU_CODE_QA_TOKEN || '';
  if (process.env.SUZAKU_APP_QA !== '1' ||
      process.env.SUZAKU_APP_QA_SUITE !== 'vscode' ||
      !process.env.SUZAKU_APP_QA_DISPLAY || process.env.DISPLAY !== process.env.SUZAKU_APP_QA_DISPLAY ||
      process.env.IBUS_ADDRESS !== 'unix:path=' + root + '/runtime/ibus.sock' ||
      !/^\/tmp\/suzaku-app-qa\.[A-Za-z0-9]{6}$/.test(root) ||
      fs.lstatSync(root).isSymbolicLink() || fs.statSync(root).uid !== process.getuid() ||
      !/^http:\/\/127\.0\.0\.1:[0-9]+$/.test(base) || !/^[a-f0-9]{32}$/.test(token)) {
    throw new Error('Refusing VS Code QA outside an owned synthetic session');
  }
  const docs = {};
  for (const field of ['editor', 'other']) {
    const file = path.join(root, field + '.txt');
    if (fs.lstatSync(file).isSymbolicLink()) throw new Error('Refusing linked QA document');
    docs[field] = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  }
  console.log('Suzaku QA: owned documents opened');
  let ack = 0, seq = 0, box, boxField, stopped = false;
  const values = {entry: '', password: ''};
  const events = [];
  const record = event => { events.push(event); if (events.length > 100) events.shift(); };
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    const field = Object.keys(docs).find(key => docs[key] === event.document);
    if (field) record({field, type: 'document-change', version: event.document.version});
  }));

  function request(route, data) {
    return new Promise((resolve, reject) => {
      const body = data === undefined ? undefined : JSON.stringify(data);
      const req = http.request(base + route + token, {
        method: body === undefined ? 'GET' : 'POST',
        headers: body === undefined ? {} : {'Content-Length': Buffer.byteLength(body)},
      }, res => {
        let response = '';
        res.setEncoding('utf8');
        res.on('data', chunk => { response += chunk; });
        res.on('end', () => {
          try {
            if (res.statusCode !== 200) throw new Error('QA bridge HTTP ' + res.statusCode);
            resolve(JSON.parse(response));
          } catch (error) { reject(error); }
        });
      });
      req.setTimeout(2000, () => req.destroy(new Error('QA bridge timeout')));
      req.on('error', reject);
      req.end(body);
    });
  }

  async function focus(command) {
    if (!['editor', 'other', 'entry', 'password'].includes(command.field)) {
      throw new Error('Unexpected QA focus target');
    }
    if (box) { box.hide(); box.dispose(); box = undefined; boxField = undefined; }
    if (docs[command.field]) {
      const editor = await vscode.window.showTextDocument(docs[command.field],
        {preview: false, preserveFocus: false});
      if (command.start !== undefined) {
        editor.selection = new vscode.Selection(editor.document.positionAt(command.start),
          editor.document.positionAt(command.end));
      }
      await vscode.commands.executeCommand('workbench.action.focusActiveEditorGroup');
    } else {
      const input = vscode.window.createInputBox();
      const field = command.field;
      box = input; boxField = field; values[field] = '';
      input.title = 'Suzaku synthetic ' + field;
      input.password = field === 'password';
      input.ignoreFocusOut = true;
      input.onDidChangeValue(value => { values[field] = value; });
      input.onDidAccept(() => record({field, type: 'accept'}));
      input.onDidHide(() => { if (box === input) boxField = undefined; });
      input.show();
    }
    ack = command.id;
  }

  async function poll() {
    if (stopped) return;
    try {
      for (const command of await request('/next/')) await focus(command);
      const activeDoc = vscode.window.activeTextEditor?.document;
      const active = boxField || Object.keys(docs).find(field => docs[field] === activeDoc) || '';
      await request('/state/', {seq: ++seq, ack, active, events,
        values: {...values, ...Object.fromEntries(Object.entries(docs).map(([key, doc]) => [key, doc.getText()]))},
        vscode: vscode.version, electron: process.versions.electron});
    } catch (error) {
      console.error('Suzaku QA observer failed:', error.message);
      stopped = true;
    } finally { if (!stopped) setTimeout(poll, 60); }
  }
  context.subscriptions.push({dispose() { stopped = true; if (box) box.dispose(); }});
  // Publish readiness before the Python driver focuses the X11 window. Initial
  // showTextDocument can otherwise wait for window focus while the driver waits
  // for this observer, a test-harness deadlock unrelated to input handling.
  poll();
};

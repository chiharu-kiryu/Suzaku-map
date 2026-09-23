#!/usr/bin/env python3
"""Standard Qt widgets; data and focus commands go only to the owned QA server."""
import importlib
import json
import os
import sys
from urllib.request import Request, urlopen

assert os.environ.get("SUZAKU_APP_QA") == "1"
assert os.environ.get("DISPLAY") == os.environ.get("SUZAKU_APP_QA_DISPLAY")
assert os.environ.get("QT_IM_MODULE") == "ibus" and os.environ.get("QT_QPA_PLATFORM") == "xcb"
assert len(sys.argv) == 4 and sys.argv[1] in ["5", "6"]
api, base, token = sys.argv[1:]
assert base.startswith("http://127.0.0.1:") and token.isalnum()
QtCore = importlib.import_module("PyQt" + api + ".QtCore")
QtGui = importlib.import_module("PyQt" + api + ".QtGui")
QtWidgets = importlib.import_module("PyQt" + api + ".QtWidgets")
Qt = QtCore.Qt
app = QtWidgets.QApplication([])
window = QtWidgets.QWidget()
window.setWindowTitle("Suzaku Qt" + api + " QA")
layout = QtWidgets.QVBoxLayout(window)
fields = {
    "editor": QtWidgets.QPlainTextEdit(), "rich": QtWidgets.QTextEdit(),
    "entry": QtWidgets.QLineEdit(), "password": QtWidgets.QLineEdit(),
    "number": QtWidgets.QLineEdit(),
}
fields["password"].setEchoMode(QtWidgets.QLineEdit.EchoMode.Password)
fields["password"].setInputMethodHints(
    Qt.InputMethodHint.ImhHiddenText | Qt.InputMethodHint.ImhSensitiveData |
    Qt.InputMethodHint.ImhNoPredictiveText)
fields["number"].setInputMethodHints(Qt.InputMethodHint.ImhFormattedNumbersOnly)
for name, field in fields.items():
    field.setObjectName(name)
    layout.addWidget(QtWidgets.QLabel(name))
    layout.addWidget(field)
window.resize(850, 680)
composition = dict.fromkeys(fields, "")
events = []
ack, seq = 0, 0


class Observer(QtCore.QObject):
    def eventFilter(self, watched, event):
        if event.type() == QtCore.QEvent.Type.InputMethod:
            name = watched.objectName()
            composition[name] = event.preeditString()
            events.append({"field": name, "type": "input-method", "text": event.commitString(),
                           "preedit": event.preeditString()})
            del events[:-200]
        return False


observer = Observer()
for field in fields.values():
    field.installEventFilter(observer)


def request(path, data=None):
    payload = None if data is None else json.dumps(data).encode()
    with urlopen(Request(base + path + "/" + token, data=payload), timeout=2) as reply:
        return json.load(reply)


def tick():
    global ack, seq
    try:
        for command in request("/next"):
            field = fields[command["field"]]
            field.setFocus(Qt.FocusReason.OtherFocusReason)
            if "start" in command:
                if isinstance(field, QtWidgets.QLineEdit):
                    field.setSelection(command["start"], command["end"] - command["start"])
                else:
                    cursor = field.textCursor()
                    cursor.setPosition(command["start"])
                    cursor.setPosition(command["end"], QtGui.QTextCursor.MoveMode.KeepAnchor)
                    field.setTextCursor(cursor)
            ack = command["id"]
        seq += 1
        request("/state", {"seq": seq, "ack": ack,
                           "active": app.focusWidget().objectName() if app.focusWidget() else "",
                           "values": {name: field.text() if isinstance(field, QtWidgets.QLineEdit)
                                      else field.toPlainText() for name, field in fields.items()},
                           "composition": composition, "events": events,
                           "qt_version": QtCore.qVersion()})
    except Exception as error:
        print("Qt fixture transport failed:", error, flush=True)
        app.exit(2)


window.show()
fields["editor"].setFocus()
timer = QtCore.QTimer()
timer.timeout.connect(tick)
timer.start(60)
sys.exit(app.exec())

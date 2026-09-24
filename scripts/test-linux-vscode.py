#!/usr/bin/env python3
"""XTest -> real VS Code/Electron -> IBus -> observed and saved owned documents."""
import importlib.util
import os
from pathlib import Path
import shutil
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("suzaku_cross_qa", Path(__file__).with_name("test-linux-cross-apps.py"))
cross = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cross)
qa, IBus = cross.qa, cross.IBus
assert os.environ["SUZAKU_APP_QA_SUITE"] == "vscode"


def check(bus, x, bridge, commits):
    qa.wait(lambda: bridge.state and x.window("Suzaku VS Code QA"), "VS Code development observer ready", timeout=40)
    window = x.window("Suzaku VS Code QA")
    x.focus(window)
    assert bus.set_global_engine("dev.suzaku.linux.ime")

    def focus(field, start=None, end=None):
        bridge.focus(field, start, end)
        qa.settle_input(bus, x, window)

    def expect(field, text):
        qa.wait(lambda: bridge.value(field) == text, f"VS Code {field} text: expected {text!r}")

    def saved(field, text):
        x.key(IBus.KEY_s, IBus.KEY_Control_L)
        qa.wait(lambda: (qa.root / (field + ".txt")).read_text() == text,
                f"VS Code saved {field}: expected {text!r}")
        expect(field, text)

    def clear(field):
        focus(field)
        assert qa.seed_is("")
        x.key(IBus.KEY_a, IBus.KEY_Control_L)
        x.key(IBus.KEY_BackSpace)
        saved(field, "")

    focus("editor")
    print("READY: VS Code", bridge.state["vscode"], "Electron", bridge.state["electron"])
    for field in ["editor", "other"]:
        clear(field)
        x.type("hello world")
        qa.wait(lambda: qa.seed_is("hello world"), "VS Code continuous draft")
        saved(field, "")
        assert qa.seed_is("hello world"), "Ctrl+S ended the draft"
        qa.wait(lambda: commits.auxiliary_visible and commits.auxiliary.startswith("Suzaku · hello world\n"),
                "VS Code draft displayed in candidate area")
        qa.commit(x)
        saved(field, "hello world")
        assert commits.texts[-1:] == ["hello world"]
        x.type(" again")
        qa.wait(lambda: qa.seed_is("again"), "empty-draft leading space reaches VS Code")
        qa.commit(x)
        saved(field, "hello world again")
        cross.passed_case(field, "Space continuity, save during composition, exact consecutive commits")

        clear(field)
        x.type("hel")
        qa.wait(lambda: qa.seed_is("hel"), "VS Code completion prefix")
        qa.choose_number(x, "hello")
        x.key(IBus.KEY_BackSpace)
        qa.wait(lambda: qa.seed_is("hel"), "VS Code completion undo")
        qa.choose_number(x, "hello")
        x.type(" world")
        qa.wait(lambda: qa.seed_is("hello world"), "VS Code completion continuation")
        saved(field, "")
        qa.commit(x)
        saved(field, "hello world")
        cross.passed_case(field, "number adoption, undo and continuation")

        focus(field, 6, 11)
        x.type("friend")
        qa.wait(lambda: qa.seed_is("friend"), "VS Code selected-text replacement draft")
        qa.commit(x)
        saved(field, "hello friend")
        focus(field, 5)
        x.type(" dear")
        qa.wait(lambda: qa.seed_is("dear"), "VS Code caret insertion draft")
        qa.commit(x)
        saved(field, "hello dear friend")
        cross.passed_case(field, "selection replacement and caret insertion")

        focus(field, 6, 10)
        x.type("cancelled")
        qa.wait(lambda: qa.seed_is("cancelled"), "VS Code selected-text cancel draft")
        expect(field, "hello dear friend")
        x.key(IBus.KEY_Escape)
        qa.wait(lambda: qa.seed_is(""), "VS Code cancel over selection")
        saved(field, "hello dear friend")
        cross.passed_case(field, "cancel preserves selected application text")

        clear(field)
        x.type("x")
        qa.wait(lambda: qa.seed_is("x"), "VS Code single-letter draft")
        x.key(IBus.KEY_BackSpace)
        qa.wait(lambda: qa.seed_is(""), "VS Code delete to empty")
        saved(field, "")
        x.type("2026, 12.5 hel")
        qa.wait(lambda: qa.seed_is("hel"), "VS Code literal numbers then prefix")
        saved(field, "2026, 12.5 ")
        qa.choose_number(x, "hello")
        qa.commit(x)
        saved(field, "2026, 12.5 hello")
        cross.passed_case(field, "delete to empty and literal numbers before candidate adoption")

    clear("editor")
    clear("other")
    focus("editor")
    x.type("discardme")
    qa.wait(lambda: qa.seed_is("discardme"), "VS Code draft before changing editor tabs")
    before = len(commits.texts)
    focus("other")
    qa.wait(lambda: qa.seed_is(""), "VS Code editor switch cancels old draft")
    expect("editor", "")
    expect("other", "")
    assert len(commits.texts) == before, "editor switch caused an implicit engine commit"
    x.type("new file")
    qa.wait(lambda: qa.seed_is("new file"), "VS Code independent second document draft")
    qa.commit(x)
    saved("other", "new file")
    cross.passed_case("switch editor tabs without leaking or committing the previous draft")

    focus("entry")
    x.type("hel")
    qa.wait(lambda: qa.seed_is("hel"), "VS Code normal InputBox prefix")
    qa.choose_number(x, "hello")
    x.type(" world")
    qa.wait(lambda: qa.seed_is("hello world"), "VS Code InputBox continuation")
    expect("entry", "")
    qa.commit(x)
    expect("entry", "hello world")
    assert not any(event["type"] == "accept" for event in bridge.state["events"])
    x.key(IBus.KEY_Return)
    qa.wait(lambda: any(event["type"] == "accept" for event in bridge.state["events"]),
            "VS Code empty-draft Enter accepts the input box")
    cross.passed_case("InputBox candidate adoption and no duplicate submit on IME Enter")

    focus("editor")
    x.type("discard before private")
    qa.wait(lambda: qa.seed_is("discard before private"), "VS Code draft before password field")
    before = len(commits.texts)
    focus("password")
    qa.wait(lambda: qa.seed_is(""), "VS Code private focus cancels old public draft")
    expect("editor", "")
    assert len(commits.texts) == before
    start = len(qa.watch.frames)
    x.type("synthetic42")
    expect("password", "synthetic42")
    assert all(not frame["seed"] and not frame["candidates"] for frame in qa.watch.frames[start:])
    assert len(commits.texts) == before, "private keys entered the engine commit path"
    cross.passed_case("password InputBox cancels old draft and keeps keys out of candidates/snapshots")

    focus("editor")
    x.type("public again")
    qa.wait(lambda: qa.seed_is("public again"), "VS Code public input resumes after password")
    qa.commit(x)
    saved("editor", "public again")
    assert not commits.preedit_updates, "default mode populated the application's preedit cache"
    cross.passed_case("public editor resumes after password without inherited text")


def main():
    bridge = None
    commits = None
    try:
        bus, x = qa.start()
        bridge = cross.Bridge()
        commits = cross.EngineCommits(bus)
        profile = qa.root / "code-profile"
        (profile / "User").mkdir(parents=True)
        shutil.copyfile(cross.fixtures / "vscode-input/settings.json", profile / "User/settings.json")
        for field in ["editor", "other"]:
            (qa.root / (field + ".txt")).touch()
        qa.spawn([
            os.environ.get("SUZAKU_APP_QA_CODE", "code"),
            "--user-data-dir=" + str(profile), "--extensions-dir=" + str(qa.root / "code-extensions"),
            "--extensionDevelopmentPath=" + str(cross.fixtures / "vscode-input"),
            "--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes",
            "--new-window", "--locale=en", "--ozone-platform=x11", "--disable-updates",
            "--password-store=basic",
            "--disable-background-networking", "--disable-component-update",
            "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1",
            "--wait", str(qa.root / "editor.txt"),
        ], env=dict(os.environ, SUZAKU_CODE_QA_BASE=bridge.base, SUZAKU_CODE_QA_TOKEN=bridge.token))
        check(bus, x, bridge, commits)
        print(f"RESULT: {cross.passed} real VS Code workflow checks passed")
    except Exception:
        print("VS Code observer state:", None if bridge is None else bridge.state)
        qa.diagnose()
        raise
    finally:
        if commits is not None:
            commits.close()
        qa.close()
        if bridge is not None:
            bridge.close()


if __name__ == "__main__":
    main()

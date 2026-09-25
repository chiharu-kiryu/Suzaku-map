#!/usr/bin/env python3
"""Read-only editor observation across two real, owned GTK processes."""
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "editor"

try:
    bus, x = qa.start()
    documents = []
    for index, text in enumerate(["first owned document", "second owned document"]):
        path = qa.root / f"suzaku-editor-{index}.txt"
        path.write_text("")
        process = qa.spawn(["gnome-text-editor", "--standalone", str(path)])
        qa.wait(lambda: x.window(path.name), "owned editor window")
        window = x.window(path.name)
        x.focus(window)
        x.click(200, 100)
        assert bus.set_global_engine("dev.suzaku.linux.ime")
        qa.settle_input(bus, x, window)
        qa.prepare_editor(x, path)
        x.type(text)
        qa.wait(lambda: qa.seed_is(text), "editor observation draft")
        qa.save_document(x, path, "")
        qa.commit(x)
        qa.save_document(x, path, text + "\n")
        documents.append((process, path, window, text))
        print("PASS: exact buffer/save observation in owned editor", index)
    for process, path, window, text in documents:
        assert process.poll() is None
        x.focus(window)
        qa.settle_input(bus, x, window)
        qa.save_document(x, path, text + "\n")
        qa.clear_document(x, path)
    print("RESULT: 2 owned editor processes, focus return and exact saved text passed")
except Exception:
    qa.diagnose()
    raise
finally:
    qa.close()

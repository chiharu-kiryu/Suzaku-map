#!/usr/bin/python3
"""Finite fake desktop commands, used only with a freshly allocated CLI fixture."""
import json
import os
from pathlib import Path
import sys
import tempfile

root = Path(os.environ["SUZAKU_REGISTRATION_QA"])
assert root.parent == Path(tempfile.gettempdir())
assert root.name.startswith("suzaku-register-cli-") and root.is_dir()
state_path = root / "desktop.json"
state = json.loads(state_path.read_text())
command = Path(sys.argv[0]).name
args = sys.argv[1:]
state["calls"].append([command, *args])
result = 0
output = ""
if command == "ibus":
    if args == ["engine"]:
        output = state["engine"]
    elif args[:1] == ["engine"] and len(args) == 2:
        state["engine"] = args[1]
    elif args == ["list-engine"]:
        output = "dev.suzaku.linux.ime - Suzaku"
    elif args == ["address"]:
        output = "unix:path=/synthetic/no-desktop-access"
    else:
        result = 1
elif command == "gsettings":
    if args[:1] == ["get"]:
        output = state["sources"]
    elif args[:1] == ["set"] and len(args) == 4:
        state["sources"] = args[3]
    else:
        result = 1
elif command == "systemctl":
    assert args[:1] == ["--user"]
    if args[1:2] == ["show"]:
        output = "255"
        result = 1 if state.get("manager_unavailable", False) else 0
    elif args[1:2] == ["enable"]:
        state["enabled"] = True
    elif args[1:2] == ["restart"]:
        state["active"] = True
        # Reproduce a registration that changes the current engine temporarily.
        state["engine"] = "dev.suzaku.linux.ime"
    elif args[1:2] == ["disable"]:
        state["enabled"] = state["active"] = False
    elif args[1:2] == ["is-active"]:
        result = 0 if state["active"] else 3
    elif args[1:2] == ["is-enabled"]:
        result = 0 if state["enabled"] else 1
        output = "enabled" if state["enabled"] else "disabled"
    elif args[1:] != ["daemon-reload"]:
        result = 1
elif command == "pgrep":
    result = 0 if state["active"] else 1
elif command == "gdbus":
    output = "dev.suzaku.linux.ime"
else:
    result = 1
state_path.write_text(json.dumps(state))
if output:
    print(output)
sys.exit(result)

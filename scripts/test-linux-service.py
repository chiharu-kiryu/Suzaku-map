#!/usr/bin/env python3
"""Actual systemd user-manager recovery, only in the owned disposable container."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

assert os.environ.get("SUZAKU_SERVICE_CONTAINER_QA") == "1"
assert Path("/.dockerenv").is_file()
assert not os.environ.get("DISPLAY") and not os.environ.get("WAYLAND_DISPLAY")
assert not os.environ.get("DBUS_SESSION_BUS_ADDRESS") and not os.environ.get("IBUS_ADDRESS")

if sys.argv[1:] == ["--bootstrap"]:
    assert os.getuid() == 0
    # The runner supplies a PRIVATE cgroup namespace, not a writable host-tree
    # bind mount. Only this container's cgroup root is made writable/delegated.
    assert Path("/proc/1/cgroup").read_text().strip() == "0::/"
    assert Path("/proc/self/cgroup").read_text().strip() == "0::/"
    subprocess.run(["mount", "-o", "remount,rw", "/sys/fs/cgroup"], check=True, timeout=5)
    scope = Path("/sys/fs/cgroup/init.scope")
    scope.mkdir()
    for pid in Path("/sys/fs/cgroup/cgroup.procs").read_text().split():
        try:
            (scope / "cgroup.procs").write_text(pid)
        except ProcessLookupError:
            pass
    for group in [Path("/sys/fs/cgroup"), scope]:
        os.chown(group, 1000, 1000)
        for name in ["cgroup.procs", "cgroup.threads", "cgroup.subtree_control"]:
            os.chown(group / name, 1000, 1000)
    # sd_booted() checks this marker; it is inside the container only.
    Path("/run/systemd/system").mkdir(parents=True, exist_ok=True)
    os.setgroups([])
    os.setgid(1000)
    os.setuid(1000)
    os.execv(sys.executable, [sys.executable, __file__, "--user"])

assert sys.argv[1:] == ["--user"] and os.getuid() == 1000
assert Path("/proc/self/cgroup").read_text().strip() == "0::/init.scope"
status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines())
assert int(status["CapEff"], 16) == 0 and int(status["CapPrm"], 16) == 0
import gi
gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus

processes, logs = [], []
passed = 0
manager = None
fixture = tempfile.TemporaryDirectory(prefix="suzaku-register-cli-")
root = Path(fixture.name)
runtime = root / "runtime"
runtime.mkdir(mode=0o700)
units = root / "config/systemd/user"
units.mkdir(parents=True)
qa_home = root / "home 朱雀"
qa_home.mkdir()
env = dict(os.environ, HOME=str(qa_home), XDG_RUNTIME_DIR=str(runtime),
           XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
           XDG_CACHE_HOME=str(root / "cache"), SUZAKU_IME_CONFIG=str(root / "ime.json"),
           SUZAKU_LINUX_IME_SOCKET=str(runtime / "suzaku-ime/host.sock"),
           IBUS_ADDRESS=f"unix:path={runtime}/ibus.sock", GSETTINGS_BACKEND="memory",
           DBUS_SESSION_BUS_ADDRESS=f"unix:path={runtime}/session-bus",
           SYSTEMD_UNIT_PATH=str(units), SYSTEMD_GENERATOR_PATH="",
           SYSTEMD_ENVIRONMENT_GENERATOR_PATH="", LC_ALL="C.UTF-8")
os.environ.update(env)
(root / "ime.json").write_text('{"language":"en","llm_enabled":false}')
unit_name = "suzaku-ibus.service"
unit_path = units / unit_name


def run(args, **kwargs):
    result = subprocess.run(args, env=env, capture_output=True, text=True, timeout=8, **kwargs)
    assert result.returncode == 0, (args, result.stdout, result.stderr)
    return result.stdout.strip()


def spawn(args):
    log = (root / f"process-{len(processes)}.log").open("w")
    logs.append(log)
    child = subprocess.Popen(args, env=env, stdout=log, stderr=log)
    processes.append(child)
    return child


def wait(check, label, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        while GLib.MainContext.default().iteration(False):
            pass
        if check():
            return
        time.sleep(0.04)
    raise AssertionError(label)


def ctl(*args):
    return run(["/usr/bin/systemctl", "--user", *args])


def state():
    output = ctl("show", "--property=ActiveState,SubState,Result,NRestarts,MainPID", unit_name)
    return dict(line.split("=", 1) for line in output.splitlines())


def command(request):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(1)
        client.connect(env["SUZAKU_LINUX_IME_SOCKET"])
        client.sendall(request.encode())
        client.shutdown(socket.SHUT_WR)
        data = b""
        while part := client.recv(65536):
            data += part
            if request == "W" and b"\n" in data:
                return json.loads(data.split(b"\n", 1)[0])
        return data


def ready():
    try:
        return command("Q") == b"1"
    except OSError:
        return False


def stable(check, label, seconds=3):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        assert check(), label
        time.sleep(0.08)


def start_bus():
    endpoint = runtime / "ibus.sock"
    if endpoint.exists():
        assert endpoint.is_socket() and not endpoint.is_symlink()
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(1)
            try:
                client.connect(str(endpoint))
            except ConnectionRefusedError:
                pass
            else:
                raise AssertionError("refusing to replace a live IBus endpoint")
        endpoint.unlink()
    child = spawn(["ibus-daemon", "--single", "--panel=disable", "--config=disable",
                   "--emoji-extension=disable", "--cache=none", "--address=" + env["IBUS_ADDRESS"]])
    wait(endpoint.exists, "private IBus endpoint")
    wait(bus.is_connected, "private IBus observer connected")
    assert child.poll() is None
    return child


def stop_bus(child):
    child.terminate()
    child.wait(timeout=3)
    wait(lambda: not bus.is_connected(), "private IBus observer disconnected")


def passed_case(label):
    global passed
    passed += 1
    print("PASS:", label, flush=True)


def type_draft(seed):
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    target = bus.create_input_context("SuzakuServiceQA")
    target.set_capabilities(IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT |
                            IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.AUXILIARY_TEXT)
    target.focus_in()
    target.set_engine("dev.suzaku.linux.ime")
    last, since = None, time.monotonic()

    def target_ready():
        nonlocal last, since
        frame = command("W")
        current = (bus.current_input_context(), frame["context"], frame["focused"])
        if current != last:
            last, since = current, time.monotonic()
        return (current[0] == target.get_object_path() and current[2] and
                time.monotonic() - since >= 0.5)

    wait(target_ready, "private native input target settles after engine activation")
    for character in seed:
        assert target.process_key_event(ord(character), 0, 0)
    frame = command("W")
    assert frame["seed"] == seed and frame["candidates"], (seed, frame)
    return target, frame


try:
    # The registration CLI is real; only its desktop discovery/activation calls
    # are replaced. The generated unit is then loaded by REAL systemd below.
    commands = root / "commands"
    commands.mkdir()
    for name in ["systemctl", "ibus", "gsettings", "pgrep", "gdbus"]:
        shutil.copyfile("/checks/fixtures/linux-registration-command.py", commands / name)
        (commands / name).chmod(0o755)
    (root / "desktop.json").write_text(json.dumps({
        "engine": "xkb:us::eng", "sources": "[('xkb', 'us')]",
        "active": False, "enabled": False, "calls": []}))
    registration = dict(env, PATH=str(commands), SUZAKU_REGISTRATION_QA=str(root),
                        SUZAKU_LINUX_IME_FRAMEWORK="ibus",
                        SUZAKU_LINUX_IME_HOST_BIN="/build/linux_ime_host")
    result = subprocess.run(["/build/suzaku_tool", "linux-register", "install"],
                            env=registration, capture_output=True, text=True, timeout=15)
    assert result.returncode == 0, (result.stdout, result.stderr)
    fixed_unit = unit_path.read_text()
    assert "StartLimitIntervalSec=0\n" in fixed_unit and "RestartSec=2\n" in fixed_unit
    legacy_unit = fixed_unit.replace("StartLimitIntervalSec=0\n", "").replace("RestartSec=2\n", "RestartSec=1\n")
    unit_path.write_text(legacy_unit)
    # A minimal private target tree avoids starting distribution/user services.
    for name in ["default.target", "basic.target", "shutdown.target", "graphical-session.target"]:
        (units / name).write_text("[Unit]\nDescription=Owned service QA target\nDefaultDependencies=no\n")
    dropin = units / (unit_name + ".d")
    dropin.mkdir()
    (dropin / "logging.conf").write_text(
        f"[Service]\nStandardOutput=append:{root}/host.log\nStandardError=append:{root}/host.log\n")
    spawn(["dbus-daemon", "--session", "--nofork", "--address=" + env["DBUS_SESSION_BUS_ADDRESS"]])
    wait(lambda: (runtime / "session-bus").exists(), "private session bus")
    manager = spawn(["/usr/lib/systemd/systemd", "--user", "--unit=default.target",
                     "--log-target=console", "--log-level=warning"])

    def manager_ready():
        assert manager.poll() is None, "private systemd user manager exited"
        return (runtime / "systemd/private").exists()

    wait(manager_ready, "private systemd user manager")
    defaults = ctl("show", "--property=DefaultStartLimitIntervalUSec,DefaultStartLimitBurst")
    assert "DefaultStartLimitIntervalUSec=10s" in defaults and "DefaultStartLimitBurst=5" in defaults
    print("QA manager:", ctl("show", "--property=Version", "--value"), flush=True)
    IBus.init()
    bus = IBus.Bus.new()
    assert not bus.is_connected()
    ctl("start", unit_name)
    # Ubuntu's 255 build can retain the last process's Result=exit-code when
    # its restart job hits the limit. Verify the actual terminal state/count,
    # then absence of recovery past the rate-limit interval, not just an enum.
    wait(lambda: state()["ActiveState"] == "failed" and int(state()["NRestarts"]) == 5,
         "legacy policy exhausts retries")
    before = state()
    assert before["ActiveState"] == "failed" and int(before["NRestarts"]) >= 4
    daemon = start_bus()
    stable(lambda: state() == before and not ready(), "legacy service unexpectedly recovered", seconds=6)
    passed_case("legacy unit exhausts its five-start allowance and stays failed after IBus returns")

    ctl("stop", unit_name)
    stop_bus(daemon)
    unit_path.write_text(fixed_unit)
    ctl("daemon-reload")
    ctl("reset-failed", unit_name)
    policy = ctl("show", "--property=Restart,RestartUSec,StartLimitIntervalUSec", unit_name)
    assert "Restart=always" in policy and "RestartUSec=2s" in policy and "StartLimitIntervalUSec=0" in policy
    ctl("start", unit_name)
    wait(lambda: int(state()["NRestarts"]) >= 6, "fixed policy survives a long startup outage", timeout=20)
    assert state()["ActiveState"] != "failed" and not ready()
    daemon = start_bus()
    wait(ready, "systemd starts the host automatically when IBus appears")
    assert state()["ActiveState"] == "active"
    passed_case("fixed unit keeps retrying during long startup outage and becomes ready automatically")

    context, old = type_draft("hel")
    retries = int(state()["NRestarts"])
    stop_bus(daemon)
    wait(lambda: int(state()["NRestarts"]) >= retries + 6,
         "fixed policy keeps retrying after a long daemon outage", timeout=20)
    assert state()["ActiveState"] != "failed" and not ready()
    daemon = start_bus()
    wait(ready, "systemd reconnects the host without another start command")
    fresh = command("W")
    assert fresh["host"] != old["host"] and not fresh["seed"] and not fresh["candidates"]
    context, fresh = type_draft("fresh")
    commits = []
    context.connect("commit-text", lambda _context, text: commits.append(text.get_text()))
    assert command(f'A{old["host"]} {fresh["revision"]} K0') == b"0"
    assert command("W") == fresh and not commits
    assert command(f'A{fresh["host"]} {fresh["revision"]} K0') == b"1"
    wait(lambda: commits == [fresh["candidates"][0]["text"]], "recovered service commits exact fresh text")
    assert not command("W")["seed"]
    passed_case("systemd recovers from daemon loss without replaying old composition or host tokens")

    ctl("stop", unit_name)
    stable(lambda: state()["ActiveState"] == "inactive" and not ready(), "explicit stop was undone")
    passed_case("explicit stop stays stopped while IBus remains available")

    stop_bus(daemon)
    ctl("start", unit_name)
    wait(lambda: state()["SubState"] == "auto-restart", "retry scheduled while IBus is absent")
    ctl("stop", unit_name)
    daemon = start_bus()
    stable(lambda: state()["ActiveState"] == "inactive" and not ready(), "queued retry survived stop")
    passed_case("stop cancels pending retries and later IBus recovery does not restart the host")
    assert passed == 5
    print("RESULT: 5 strict real-systemd recovery checks passed", flush=True)
except Exception:
    if manager is not None and manager.poll() is None:
        print("QA final unit state:", state(), flush=True)
        print("QA loaded policy:", ctl("show", "--property=Restart,RestartUSec,StartLimitIntervalUSec,StartLimitBurst", unit_name), flush=True)
    for path in sorted(root.glob("*.log")):
        print("QA log:", path.name, path.read_text()[-8000:], flush=True)
    raise
finally:
    if manager is not None and manager.poll() is None:
        try:
            ctl("stop", unit_name)
        except (AssertionError, subprocess.TimeoutExpired):
            pass
    for child in reversed(processes):
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=3)
    for log in logs:
        log.close()
    fixture.cleanup()

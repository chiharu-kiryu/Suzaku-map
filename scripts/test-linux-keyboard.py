#!/usr/bin/env python3
"""Physical XTest keys, XKB locks/layouts and real GTK input in an owned display."""
import ctypes as C
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)  # Validate private session/display/config BEFORE any XKB changes.
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "keyboard"
IBus = qa.IBus
passed = 0


class State(C.Structure):
    # XkbStateRec from X11/extensions/XKBstr.h, including its ABI padding.
    _fields_ = [("group", C.c_ubyte), ("locked_group", C.c_ubyte),
                ("base_group", C.c_ushort), ("latched_group", C.c_ushort)] + [
                    (name, C.c_ubyte) for name in
                    ("mods", "base_mods", "latched_mods", "locked_mods", "compat_state",
                     "grab_mods", "compat_grab_mods", "lookup_mods", "compat_lookup_mods")
                ] + [("ptr_buttons", C.c_ushort)]


def layout(name="us", variant=""):
    qa.subprocess.run(["setxkbmap", "-layout", name, "-variant", variant,
                       "-option", "", "-option", "compose:menu"], check=True, timeout=3)
    qa.pump()


class Keyboard:
    def __init__(self, x):
        self.x = x
        for name, args, result in [
            ("XkbGetState", [C.c_void_p, C.c_uint, C.POINTER(State)], C.c_int),
            ("XkbGetIndicatorState", [C.c_void_p, C.c_uint, C.POINTER(C.c_uint)], C.c_int),
            ("XkbKeysymToModifiers", [C.c_void_p, C.c_ulong], C.c_uint),
            ("XkbGetNamedIndicator", [C.c_void_p, C.c_ulong, C.POINTER(C.c_int),
                                     C.POINTER(C.c_int), C.c_void_p, C.POINTER(C.c_int)], C.c_int),
        ]:
            function = getattr(x.x, name)
            function.argtypes, function.restype = args, result
        # Capture physical positions under US once. Later layout changes must be
        # resolved by XKB/GTK, NOT by turning desired Unicode into fake keyvals.
        symbols = list("abcdefghijklmnopqrstuvwxyz0123456789'.,/;`-=[]\\ ") + [
            "Shift_L", "Shift_R", "Control_L", "Alt_L", "Alt_R", "Caps_Lock", "Num_Lock",
            "Return", "KP_Enter", "BackSpace", "Escape", "Tab", "Menu", "F11",
            "KP_Add", "KP_Subtract", "KP_Multiply", "KP_Divide", "KP_Decimal",
        ] + [f"KP_{digit}" for digit in range(10)]
        self.codes = {}
        for name in symbols:
            symbol = ord(name) if len(name) == 1 else getattr(IBus, "KEY_" + name)
            code = x.x.XKeysymToKeycode(x.display, symbol)
            assert code, (name, symbol)
            self.codes[name] = code
        self.caps = x.x.XkbKeysymToModifiers(x.display, IBus.KEY_Caps_Lock)
        self.num = x.x.XkbKeysymToModifiers(x.display, IBus.KEY_Num_Lock)
        assert self.caps and self.num and not (self.caps & self.num)

    def state(self):
        state, leds = State(), C.c_uint()
        assert self.x.x.XkbGetState(self.x.display, 0x100, C.byref(state)) == 0
        assert self.x.x.XkbGetIndicatorState(self.x.display, 0x100, C.byref(leds)) == 0
        assert state.base_mods == 0 and state.latched_mods == 0, "stuck physical modifier"
        for name, mask in [(b"Caps Lock", self.caps), (b"Num Lock", self.num)]:
            atom = self.x.x.XInternAtom(self.x.display, name, 1)
            index, lit = C.c_int(), C.c_int()
            assert atom and self.x.x.XkbGetNamedIndicator(
                self.x.display, atom, C.byref(index), C.byref(lit), None, None)
            assert 0 <= index.value < 32
            assert bool(lit.value) == bool(leds.value & (1 << index.value))
            assert bool(lit.value) == bool(state.locked_mods & mask), "XKB lock/indicator mismatch"
        return state.locked_mods, leds.value, state.group

    def press(self, name, *modifiers):
        pressed = []
        try:
            for key in [*modifiers, name]:
                code = self.codes[key]
                assert self.x.xt.XTestFakeKeyEvent(self.x.display, code, 1, 0)
                pressed.append(code)
            self.x.x.XSync(self.x.display, 0)
            qa.time.sleep(.015)
        finally:
            for code in reversed(pressed):
                assert self.x.xt.XTestFakeKeyEvent(self.x.display, code, 0, 0)
            self.x.x.XSync(self.x.display, 0)
        qa.time.sleep(.025)
        qa.pump()

    def type(self, text, *modifiers):
        for name in text:
            self.press(name, *modifiers)

    def locks(self, caps=False, num=False):
        for enabled, mask, name in [(caps, self.caps, "Caps_Lock"), (num, self.num, "Num_Lock")]:
            if bool(self.state()[0] & mask) != enabled:
                self.press(name)
            assert bool(self.state()[0] & mask) == enabled

    def map(self):
        return qa.subprocess.check_output(["xkbcomp", "-xkb", qa.os.environ["DISPLAY"], "-"],
                                          stderr=qa.subprocess.DEVNULL, timeout=3)


def expect(seed):
    qa.wait(lambda: qa.seed_is(seed), f"physical keys must produce draft {seed!r}")


def clean():
    # Housekeeping shortcuts use US; no personal display/layout is ever connected.
    layout()
    keyboard.locks()
    keyboard.press("Escape")
    expect("")
    qa.clear_document(x, document)


def finish(seed, label, initial, keymap, enter="Return"):
    global passed
    expect(seed)
    qa.save_document(x, document, "")
    expect(seed)  # Ctrl+S is an application shortcut, never draft submission.
    assert keyboard.state() == initial and keyboard.map() == keymap
    keyboard.press(enter)
    expect("")
    qa.save_document(x, document, seed + "\n")
    assert keyboard.state() == initial and keyboard.map() == keymap
    assert x.focused() == window and editor.poll() is None
    passed += 1
    print("PASS:", label, flush=True)


try:
    layout()
    bus, x = qa.start()
    keyboard = Keyboard(x)
    document = qa.root / "suzaku-keyboard-qa.txt"
    document.write_text("")
    editor = qa.spawn(["gnome-text-editor", "--standalone", str(document)])
    qa.wait(lambda: x.window(document.name), "physical keyboard QA editor")
    window = x.window(document.name)
    x.focus(window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    qa.settle_input(bus, x, window)
    qa.prepare_editor(x, document)

    for caps in [False, True]:
        for shift in [None, "Shift_L", "Shift_R"]:
            clean()
            keyboard.locks(caps=caps)
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type("abc", *([shift] if shift else []))
            expected = "ABC" if caps != bool(shift) else "abc"
            finish(expected, f"Caps={caps}, Shift={shift}: case and lock indicators stay aligned", initial, keymap)

    for caps in [False, True]:
        for num in [False, True]:
            clean()
            keyboard.locks(caps=caps, num=num)
            initial, keymap = keyboard.state(), keyboard.map()
            keyboard.type("hel")
            original = "HEL" if caps else "hel"
            expect(original)
            adopted = "HELLO" if caps else "hello"
            qa.choose_number(x, adopted)
            qa.save_document(x, document, "")
            keyboard.press("BackSpace")
            expect(original)
            qa.choose_number(x, adopted)
            keyboard.press(" ")
            finish(adopted + " ", f"Caps={caps}, Num={num}: number adoption, undo and Space continuation", initial, keymap)

    clean()
    keyboard.locks(num=True)
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    for digit in range(10):
        keyboard.press(f"KP_{digit}")
    for key in ["KP_Decimal", "KP_Add", "KP_Subtract", "KP_Multiply", "KP_Divide"]:
        keyboard.press(key)
    finish("v 0123456789.+-*/", "NumLock keypad digits/operators stay literal inside a draft", initial, keymap, "KP_Enter")

    clean()
    keyboard.locks(num=True)
    initial, keymap = keyboard.state(), keyboard.map()
    for key in ["KP_1", "KP_2", "KP_Decimal", "KP_5"]:
        keyboard.press(key)
    expect("")
    qa.save_document(x, document, "12.5\n")
    assert keyboard.state() == initial and keyboard.map() == keymap
    passed += 1
    print("PASS: empty-draft keypad digits/decimal go straight to the application", flush=True)

    # Keys below refer to their US physical position. Actual symbols come from
    # the named keyboard layout, with no direct IBus ProcessKeyEvent injection.
    cases = [
        ("us", "", [("a", ()), ("1", ("Shift_L",)), ("2", ("Shift_R",))], "a!@"),
        ("gb", "", [("a", ()), ("2", ("Shift_L",)), ("3", ("Shift_R",))], 'a"£'),
        ("de", "", [("y", ()), ("z", ()), ("q", ("Alt_R",)), ("e", ("Alt_R",)),
                      ("7", ("Alt_R",)), ("0", ("Alt_R",))], "zy@€{}"),
        ("fr", "", [("q", ()), ("a", ()), ("2", ()), ("2", ("Shift_L",)),
                      ("0", ("Alt_R",))], "aqé2@"),
        ("us", "intl", [("c", ()), ("a", ()), ("f", ()), ("'", ()), ("e", ()),
                           (" ", ()), ("'", ("Shift_L",)), ("u", ())], "café ü"),
    ]
    for name, variant, keys, expected in cases:
        clean()
        layout(name, variant)
        initial, keymap = keyboard.state(), keyboard.map()
        for key, modifiers in keys:
            keyboard.press(key, *modifiers)
        finish(expected, f"{name}/{variant or 'default'}: physical letters, Shift, AltGr and dead keys", initial, keymap)

    for side in ["Shift_L", "Shift_R"]:
        clean()
        initial, keymap = keyboard.state(), keyboard.map()
        keyboard.type("hel")
        expect("hel")
        keyboard.press("Tab")
        qa.wait(lambda: qa.watch.latest["selected"] == 1, "Tab selects the second row")
        keyboard.press("Tab", side)
        qa.wait(lambda: qa.watch.latest["selected"] == 0, "Shift Tab returns to first row")
        keyboard.press("Tab")
        keyboard.press("KP_Enter", side)
        expect("hello")
        keyboard.press("BackSpace")
        expect("hel")
        keyboard.type("lo world")
        keyboard.press("BackSpace", "Control_L")
        finish("hello ", f"{side}: reverse navigation, keypad adoption/undo and Ctrl Backspace", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    for digit in "0123456789":
        keyboard.press(digit, "Alt_L")
    keyboard.press("Menu")
    keyboard.press("1")
    keyboard.press("2")
    finish("v 0123456789½", "Alt literal digits and physical Compose never choose numbered candidates", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("ab")
    keyboard.locks(caps=True)
    keyboard.type("cd")
    keyboard.type("ef", "Shift_R")
    keyboard.locks()
    keyboard.type("gh")
    finish("abCDefgh", "mid-draft Caps changes preserve spelling and release both Shift states", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v")
    keyboard.locks(num=True)
    keyboard.press("KP_1")
    keyboard.press("KP_2")
    keyboard.locks()
    expect("v12")
    keyboard.locks(num=True)
    keyboard.press("KP_Decimal")
    keyboard.press("KP_5")
    keyboard.locks()
    finish("v12.5", "mid-draft NumLock changes do not choose candidates or submit", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("left ")
    layout("de")
    keyboard.press("y")
    expect("left z")
    layout("fr")
    keyboard.press("q")
    expect("left za")
    layout()
    finish("left za", "live US/German/French layout changes preserve the same draft", initial, keymap)

    for cancel in ["Escape", "BackSpace"]:
        clean()
        layout("us", "intl")
        initial, keymap = keyboard.state(), keyboard.map()
        keyboard.type("caf")
        keyboard.press("'")
        expect("caf")
        keyboard.press(cancel)
        expect("caf")
        keyboard.press("e")
        finish("cafe", f"dead-key {cancel} cancels only the unfinished accent", initial, keymap)

    clean()
    initial, keymap = keyboard.state(), keyboard.map()
    keyboard.type("v ")
    keyboard.press("Menu")
    keyboard.press("F11")
    keyboard.press("1")
    finish("v two words", "private custom Compose text stays in the editable draft", initial, keymap)

    assert passed == 26, passed
    print("RESULT: 26 strict physical-keyboard workflow groups passed", flush=True)
except Exception:
    if "keyboard" in globals():
        print("QA keyboard state:", keyboard.state())
    qa.diagnose()
    raise
finally:
    qa.close()

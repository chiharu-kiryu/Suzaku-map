---
description: Keep native settings persistence off the Linux input thread without weakening acknowledgements
applyTo: "src/ime/settings.rs,src/ime_host.rs,src/ime_host/control.rs,src/linux/ibus_engine_bridge.c,src/linux/ibus_ipc.inc.c,src/linux/ibus_language.inc.c,scripts/test-native-sync.py,scripts/fixtures/slow-settings-fsync.c"
---

# Native settings controls

Stage blocking settings reads, writes and `fsync` in a bounded worker without the shared input-session mutex; publish and apply on the IBus thread only while the owning client or focus-bound gesture, deadline and settings baseline are still valid.
Keep writer/restore leases through publication, and retain the single-worker permit until both the canceled job and its worker have exited; dropping a pending client must discard its unpublished temporary file rather than apply late.
Equal-value controls still validate and persist settings, but preserve candidate selection, Compose and adoption undo; test both this rule and failure/cancellation with the [isolated slow-I/O gate](../../CONTRIBUTING.md).
Treat an empty acknowledgement as unconfirmed, not as proof of an outdated host or of the cause of a historical timeout; see the [controlled reproduction and its limits](../../docs/bug-audit-settings-io-2026-09-27.md).

## Learnings

Native chords arrive as modifier presses, the command key, repeats and releases, not just a single key plus a modifier mask: test both complete-chord IBus calls and physical XTest sequences on an owned display.
For example, pure Ctrl/Shift presses must leave a pending Compose sequence intact until the command key is known, and a held Ctrl+Shift+Space must remain one-shot even after Ctrl/Shift are released first; never change the desktop keymap or lock LEDs to implement this.

In native fixtures, an already-empty draft is not acknowledgement of an asynchronous IBus `Reset`.
Capture the previous companion context and require both its increase and an empty draft before sending a replacement over the separate companion socket; otherwise a pending reset can erase the new prefix.
Keep the existing deadline and exact-text assertions instead of adding sleeps or retries; see the [English boundary audit](../../docs/bug-audit-english-context-2026-09-28.md).

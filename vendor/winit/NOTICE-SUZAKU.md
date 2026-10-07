# Suzaku's modified copy of winit 0.30.13

This is a locally modified copy of **winit 0.30.13**, not an unmodified upstream
release. Suzaku's modification below was made on 2026-10-07. Upstream attribution,
the Apache-2.0 license in `LICENSE`, and the original source structure are retained.

## Provenance

- Upstream: https://github.com/rust-windowing/winit
- Source: the crates.io `winit` 0.30.13 package, copied from Cargo's local registry.
- Package `.cargo_vcs_info.json` records upstream commit
  `e9809ef54b18499bb4f2cac945719ecc2a61061b` and an empty `path_in_vcs`.
- The registry-normalized `Cargo.toml`, original `Cargo.toml.orig`, and
  `.cargo_vcs_info.json` are retained. The local registry's `.cargo-ok` cache marker
  is not part of the vendored source.

## Changed files and behavior

`src/event.rs` is modified to add the local public enum variant
`WindowEvent::X11MouseButtonPosition { device_id, position }`, marked
`#[doc(hidden)]`. This is a local-fork API extension, not an upstream API. Despite
being hidden from generated documentation, downstream exhaustive matches must
account for the additional variant. The existing event-type test inventory also
includes it.

`src/platform_impl/linux/x11/event_processor.rs` is modified. For an XI2 button
event translated to `WindowEvent::MouseInput`, it now updates the cursor cache
and emits `X11MouseButtonPosition` using that **same event's** `event_x`/`event_y`
immediately before delivering `MouseInput`. It does so for every press and release,
including repeated clicks at an unchanged position. The application callback runs
after the cursor-cache lock has been released. This position update is deliberately
not `CursorMoved`: handling a queued release as motion could advance a manual drag
again after an earlier motion event has already moved the window. Applications
use the new event to refresh button hit testing, without advancing drag or ink
motion; the ordinary motion path remains unchanged.

`src/platform_impl/linux/x11/suzaku_button_position.rs` is added. It contains the
event-ordering helper used by the production path and five pure regression tests
for event ordering, repeated same-position clicks, preservation of the supplied
device/button/phase and signed fractional coordinates, and unchanged wheel-only
handling. A queued-position test also verifies that differing press/release
coordinates stay in order without producing any `CursorMoved` events. Suzaku's
root integration test includes this exact module instead of a
copy; no separate winit workspace or test dependency download is required. The
pure tests use the public dummy device ID; real device routing is covered by the
isolated actual-window regression, not simulated by fabricated device IDs.

`NOTICE-SUZAKU.md` is added to describe these modifications. No upstream license
file has been replaced.

The existing pointer-emulated-touch filter and button mapping are unchanged.
Wheel button details 4 through 7 remain wheel-only. The patch adds no connection,
pointer query, blocking I/O, input injection, or root-window subscription. The
hidden public enum extension is available on all platforms, but only the X11
backend emits it; other platform backends are unchanged.

## Reproduction and scope

An X11 window can move under a stationary pointer without a corresponding motion
event. Suzaku reproduced this on an isolated Xvfb display without a window manager:
a bottom-docked keyboard resized and moved between clicks, and a later click at
the same screen position was interpreted using an obsolete window-local cursor
position. The XI2 button event already carried the correct local coordinates,
but upstream 0.30.13 did not deliver them to the application with `MouseInput`.

Querying the pointer after receiving the event would instead report its later
position and could misinterpret queued clicks. This patch uses event-time
coordinates without such a query. It does not claim to fix all layout changes
between a press and release; the application remains responsible for keeping or
cancelling the captured interaction when its displayed scene changes.

## Maintenance and removal

Keep this delta small when updating winit. Check the XI2 button mapping, touch
filter, cursor-cache synchronization, and callback ordering against the new
upstream source. Run the pure `suzaku_button_position_tests` and Suzaku's isolated
actual-window repeated-click regression, including a no-window-manager display.
The tests are included here; their presence alone is not a claim that validation
has run or passed.

Remove the local patch and dependency override when the chosen upstream version
provides equivalent event-time coordinates for each mouse-button callback
without misclassifying them as motion (or a suitable event-time API used by
Suzaku), and the isolated regression passes without this patch. Migrate the
application's `X11MouseButtonPosition` handling at the same time. Include this
notice alongside the upstream license when redistributing the modified dependency.

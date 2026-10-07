//! Test the exact pure helper used by the vendored X11 button dispatcher.
//! No display, X server, pointer polling or input injection is required.
#![cfg(all(feature = "gpu", target_os = "linux"))]

// The shared helper imports these modules through its enclosing crate, both
// here and inside winit. Do not copy its implementation into a test fixture.
pub use winit::{dpi, event};

#[path = "../vendor/winit/src/platform_impl/linux/x11/suzaku_button_position.rs"]
mod suzaku_button_position;

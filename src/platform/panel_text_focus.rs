//! Explicit text editing for the otherwise non-activating companion window.
//! Never called when showing the panel, dragging it, or clicking candidates.
use winit::window::Window;

#[derive(Default)]
pub struct PanelTextFocus {
    #[cfg(target_os = "linux")]
    lease: Option<x11::FocusLease>,
}

impl PanelTextFocus {
    pub fn request(&mut self, window: &Window, non_focusing: bool) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        if non_focusing {
            // winit::focus_window uses _NET_ACTIVE_WINDOW on X11. Window
            // managers ignore that request for override-redirect companions.
            if self.lease.is_none() {
                self.lease = Some(x11::FocusLease::acquire(window)?);
            }
            return Ok(());
        }
        let _ = non_focusing;
        window.focus_window();
        Ok(())
    }

    /// Restore only while we still own focus. A user's later app switch wins.
    pub fn release(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(lease) = self.lease.take() {
            let _ = lease.restore();
        }
    }

    pub fn forget(&mut self) {
        #[cfg(target_os = "linux")]
        {
            self.lease = None;
        }
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use super::*;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::{
        connection::Connection,
        protocol::xproto::{ConnectionExt, InputFocus, MapState},
        rust_connection::RustConnection,
    };

    pub(super) struct FocusLease {
        connection: RustConnection,
        panel: u32,
        previous: u32,
    }

    impl FocusLease {
        pub(super) fn acquire(window: &Window) -> Result<Self, String> {
            let panel = match window.window_handle().map_err(|e| e.to_string())?.as_raw() {
                RawWindowHandle::Xlib(handle) => handle.window as u32,
                RawWindowHandle::Xcb(handle) => handle.window.get(),
                _ => return Err("This companion does not have an X11 window".into()),
            };
            let (connection, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
            let previous = connection
                .get_input_focus()
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?
                .focus;
            connection
                .set_input_focus(InputFocus::PARENT, panel, x11rb::CURRENT_TIME)
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())?;
            connection.flush().map_err(|e| e.to_string())?;
            Ok(Self {
                connection,
                panel,
                previous,
            })
        }

        pub(super) fn restore(self) -> Result<(), Box<dyn std::error::Error>> {
            if self.connection.get_input_focus()?.reply()?.focus != self.panel {
                return Ok(());
            }
            if self.previous == self.panel {
                return Ok(());
            }
            // A destroyed/hidden target must never leave the user with no
            // keyboard focus. Let the server revert to its pointer root instead.
            let previous = if self.previous > 1
                && self
                    .connection
                    .get_window_attributes(self.previous)?
                    .reply()
                    .is_ok_and(|attributes| attributes.map_state == MapState::VIEWABLE)
            {
                self.previous
            } else {
                u32::from(InputFocus::POINTER_ROOT)
            };
            self.connection
                .set_input_focus(InputFocus::POINTER_ROOT, previous, x11rb::CURRENT_TIME)?
                .check()?;
            self.connection.flush()?;
            Ok(())
        }
    }
}

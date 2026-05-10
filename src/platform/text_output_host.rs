#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostTextOutputStatus {
    Delivered,
    PermissionRequired,
    Unsupported,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostTextOutputResult {
    pub status: HostTextOutputStatus,
    pub message: String,
}

impl HostTextOutputResult {
    fn delivered(message: impl Into<String>) -> Self {
        Self {
            status: HostTextOutputStatus::Delivered,
            message: message.into(),
        }
    }

    fn permission_required(message: impl Into<String>) -> Self {
        Self {
            status: HostTextOutputStatus::PermissionRequired,
            message: message.into(),
        }
    }

    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            status: HostTextOutputStatus::Unsupported,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            status: HostTextOutputStatus::Error,
            message: message.into(),
        }
    }

    pub fn delivered_successfully(&self) -> bool {
        self.status == HostTextOutputStatus::Delivered
    }
}

pub fn commit_text_to_active_target(text: &str) -> HostTextOutputResult {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return HostTextOutputResult::error("Nothing to send to the active app.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        return macos_commit_text(trimmed);
    }

    #[cfg(target_os = "windows")]
    {
        return HostTextOutputResult::unsupported(
            "Windows host text output is not wired yet.".to_string(),
        );
    }

    #[cfg(target_os = "linux")]
    {
        return HostTextOutputResult::unsupported(
            "Linux host text output is not wired yet.".to_string(),
        );
    }

    #[allow(unreachable_code)]
    HostTextOutputResult::unsupported("Host text output is unavailable on this platform.")
}

#[cfg(target_os = "macos")]
fn macos_commit_text(text: &str) -> HostTextOutputResult {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};

    const SUZAKU_TEXT_OUTPUT_UNAVAILABLE: c_int = 0;
    const SUZAKU_TEXT_OUTPUT_READY: c_int = 1;
    const SUZAKU_TEXT_OUTPUT_PERMISSION_REQUIRED: c_int = 2;
    const SUZAKU_TEXT_OUTPUT_ERROR: c_int = 3;

    unsafe extern "C" {
        fn suzaku_text_output_permission_state(prompt: bool) -> c_int;
        fn suzaku_text_output_commit_utf8(text: *const c_char) -> bool;
    }

    let permission = unsafe { suzaku_text_output_permission_state(false) };
    if permission == SUZAKU_TEXT_OUTPUT_PERMISSION_REQUIRED {
        let _ = unsafe { suzaku_text_output_permission_state(true) };
        return HostTextOutputResult::permission_required(
            "Enable Accessibility for Suzaku Panel to send text to the active app.".to_string(),
        );
    }
    if permission == SUZAKU_TEXT_OUTPUT_UNAVAILABLE {
        return HostTextOutputResult::unsupported(
            "Host text output is unavailable in the current macOS runtime.".to_string(),
        );
    }
    if permission == SUZAKU_TEXT_OUTPUT_ERROR {
        return HostTextOutputResult::error(
            "macOS text output bridge hit an error before sending text.".to_string(),
        );
    }
    if permission != SUZAKU_TEXT_OUTPUT_READY {
        return HostTextOutputResult::error(
            "macOS text output bridge returned an unknown state.".to_string(),
        );
    }

    let sanitized = text.replace('\0', " ");
    let Ok(c_text) = CString::new(sanitized) else {
        return HostTextOutputResult::error(
            "Committed text contained an unsupported null byte.".to_string(),
        );
    };

    if unsafe { suzaku_text_output_commit_utf8(c_text.as_ptr()) } {
        HostTextOutputResult::delivered("Sent to active app.".to_string())
    } else {
        HostTextOutputResult::error(
            "Could not paste into the active app. Check Accessibility permission.".to_string(),
        )
    }
}

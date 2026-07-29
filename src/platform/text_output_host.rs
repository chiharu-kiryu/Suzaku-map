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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_variants_encode_expected_statuses() {
        let delivered = HostTextOutputResult::delivered("ok");
        let permission_required = HostTextOutputResult::permission_required("perm");
        let unsupported = HostTextOutputResult::unsupported("no");
        let error = HostTextOutputResult::error("bad");

        assert!(delivered.delivered_successfully());
        assert_eq!(
            permission_required.status,
            HostTextOutputStatus::PermissionRequired
        );
        assert_eq!(unsupported.status, HostTextOutputStatus::Unsupported);
        assert_eq!(error.status, HostTextOutputStatus::Error);
        assert_eq!(delivered.message, "ok");
    }

    #[test]
    fn commit_text_rejects_empty_or_whitespace_text() {
        let empty = commit_text_to_active_target("");
        let blank = commit_text_to_active_target("   \n\t");

        assert_eq!(empty.status, HostTextOutputStatus::Error);
        assert_eq!(
            empty.message,
            "Nothing to send to the active app.".to_string()
        );
        assert_eq!(blank.status, HostTextOutputStatus::Error);
        assert_eq!(blank.message, empty.message);
    }

    #[test]
    fn delivered_successfully_rejects_non_delivered_variants() {
        let permission_required = HostTextOutputResult::permission_required("perm");
        let unsupported = HostTextOutputResult::unsupported("no");
        let error = HostTextOutputResult::error("bad");

        assert!(!permission_required.delivered_successfully());
        assert!(!unsupported.delivered_successfully());
        assert!(!error.delivered_successfully());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn commit_text_reports_unsupported_when_active_target_not_wired() {
        let result = commit_text_to_active_target("hello");
        assert_eq!(result.status, HostTextOutputStatus::Unsupported);
        assert!(result.message.contains("not wired yet"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn commit_text_with_null_bytes_still_reports_not_wired_on_non_macos() {
        let result = commit_text_to_active_target("has\0null");

        assert_eq!(result.status, HostTextOutputStatus::Unsupported);
        assert!(result.message.contains("not wired"));
    }
}

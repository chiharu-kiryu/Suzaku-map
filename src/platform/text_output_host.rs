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

#[cfg(target_os = "linux")]
const LINUX_IME_IPC_MAX_TEXT_BYTES: usize = 65_534;

impl HostTextOutputResult {
    #[cfg(any(target_os = "macos", target_os = "linux", test))]
    fn delivered(message: impl Into<String>) -> Self {
        Self {
            status: HostTextOutputStatus::Delivered,
            message: message.into(),
        }
    }

    #[cfg(any(target_os = "macos", test))]
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
        return linux_commit_text(trimmed);
    }

    #[allow(unreachable_code)]
    HostTextOutputResult::unsupported("Host text output is unavailable on this platform.")
}

#[cfg(target_os = "linux")]
fn linux_commit_text(text: &str) -> HostTextOutputResult {
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let sanitized = text.replace('\0', " ");
    if sanitized.len() > LINUX_IME_IPC_MAX_TEXT_BYTES {
        return HostTextOutputResult::error(format!(
            "Text is too large for the Suzaku IBus channel ({} byte limit).",
            LINUX_IME_IPC_MAX_TEXT_BYTES
        ));
    }

    let Some(socket_path) = linux_ime_socket_path() else {
        return HostTextOutputResult::unsupported(
            "XDG_RUNTIME_DIR is unavailable; cannot reach the Suzaku IBus host.".to_string(),
        );
    };
    let mut stream = match UnixStream::connect(&socket_path) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::ConnectionRefused
            ) =>
        {
            return HostTextOutputResult::unsupported(format!(
                "Suzaku IBus host is unavailable at {}.",
                socket_path.display()
            ));
        }
        Err(error) => {
            return HostTextOutputResult::error(format!(
                "Could not connect to the Suzaku IBus host: {error}"
            ));
        }
    };
    let timeout = Some(Duration::from_millis(350));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);

    let mut request = Vec::with_capacity(sanitized.len() + 1);
    request.push(b'C');
    request.extend_from_slice(sanitized.as_bytes());
    if let Err(error) = stream.write_all(&request) {
        return HostTextOutputResult::error(format!(
            "Could not send text to the Suzaku IBus host: {error}"
        ));
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);

    let mut response = [0_u8; 1];
    if let Err(error) = stream.read_exact(&mut response) {
        return HostTextOutputResult::error(format!(
            "Suzaku IBus host did not acknowledge the commit: {error}"
        ));
    }
    if response[0] == b'1' {
        HostTextOutputResult::delivered("Sent to the active Suzaku IBus target.")
    } else {
        HostTextOutputResult::unsupported(
            "No active Suzaku IBus target is available; select Suzaku in the target app first.",
        )
    }
}

#[cfg(target_os = "linux")]
fn linux_ime_socket_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("SUZAKU_LINUX_IME_SOCKET") {
        return Some(std::path::PathBuf::from(path));
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .map(|directory| directory.join("suzaku-ime/host.sock"))
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

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[test]
    fn commit_text_reports_unsupported_when_active_target_not_wired() {
        let result = commit_text_to_active_target("hello");
        assert_eq!(result.status, HostTextOutputStatus::Unsupported);
        assert!(result.message.contains("not wired yet"));
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[test]
    fn commit_text_with_null_bytes_still_reports_not_wired_on_non_macos() {
        let result = commit_text_to_active_target("has\0null");

        assert_eq!(result.status, HostTextOutputStatus::Unsupported);
        assert!(result.message.contains("not wired"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_commit_reports_unavailable_when_host_socket_is_missing() {
        crate::platform::test_env::with_test_env(|env| {
            let socket = unique_linux_test_socket("missing");
            env.set_var(
                "SUZAKU_LINUX_IME_SOCKET",
                socket.to_str().expect("socket path"),
            );

            let result = commit_text_to_active_target("hello");

            assert_eq!(result.status, HostTextOutputStatus::Unsupported);
            assert!(result.message.contains("unavailable"));
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_commit_round_trips_through_local_host_socket() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;

        crate::platform::test_env::with_test_env(|env| {
            let socket = unique_linux_test_socket("delivered");
            let listener = UnixListener::bind(&socket).expect("bind mock IBus host");
            env.set_var(
                "SUZAKU_LINUX_IME_SOCKET",
                socket.to_str().expect("socket path"),
            );
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept panel client");
                let mut request = Vec::new();
                stream
                    .read_to_end(&mut request)
                    .expect("read commit request");
                stream.write_all(b"1").expect("acknowledge commit");
                request
            });

            let result = commit_text_to_active_target("has\0null");
            let request = server.join().expect("join mock IBus host");

            assert_eq!(result.status, HostTextOutputStatus::Delivered);
            assert_eq!(request, b"Chas null");
            std::fs::remove_file(socket).expect("remove mock socket");
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_commit_rejects_text_larger_than_the_ipc_frame() {
        let text = "x".repeat(LINUX_IME_IPC_MAX_TEXT_BYTES + 1);

        let result = linux_commit_text(&text);

        assert_eq!(result.status, HostTextOutputStatus::Error);
        assert!(result.message.contains("too large"));
    }

    #[cfg(target_os = "linux")]
    fn unique_linux_test_socket(label: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "suzaku-text-output-{label}-{}-{nonce}.sock",
            std::process::id()
        ))
    }
}

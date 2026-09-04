use std::ffi::CString;

use super::linux_ime::{recommended_component_name, recommended_connection_name};

unsafe extern "C" {
    fn suzaku_linux_ibus_run(
        component_name: *const std::os::raw::c_char,
        engine_name: *const std::os::raw::c_char,
        version: *const std::os::raw::c_char,
    ) -> std::os::raw::c_int;
    fn suzaku_linux_ibus_probe_roundtrip(
        engine_name: *const std::os::raw::c_char,
        seed: *const std::os::raw::c_char,
        via_ipc: bool,
        committed: *mut std::os::raw::c_char,
        committed_capacity: usize,
    ) -> std::os::raw::c_int;
}

pub fn probe_roundtrip(seed: &str) -> Result<String, String> {
    probe_roundtrip_mode(seed, false)
}

pub fn probe_ipc_roundtrip(text: &str) -> Result<String, String> {
    probe_roundtrip_mode(text, true)
}

fn probe_roundtrip_mode(seed: &str, via_ipc: bool) -> Result<String, String> {
    let engine_name = CString::new(recommended_connection_name())
        .map_err(|_| "IBus engine name contains a null byte".to_string())?;
    let seed = CString::new(seed).map_err(|_| "Probe seed contains a null byte".to_string())?;
    let mut committed = vec![0_u8; 4096];
    let code = unsafe {
        suzaku_linux_ibus_probe_roundtrip(
            engine_name.as_ptr(),
            seed.as_ptr(),
            via_ipc,
            committed.as_mut_ptr().cast(),
            committed.len(),
        )
    };
    if code != 0 {
        let detail = match code {
            1 => "invalid probe input",
            2 => "IBus daemon unavailable",
            3 => "could not create an isolated input context",
            4 => "IBus did not expose a restorable current engine",
            5 => "could not switch the isolated context to Suzaku",
            6 => "Suzaku engine did not attach to the probe context",
            7 => "Suzaku rejected a seed key event",
            8 => "Suzaku rejected the commit key event",
            9 => "the client did not receive CommitText",
            10 => "the previous IBus engine could not be restored",
            11 => "the panel IPC channel rejected the commit",
            _ => "unknown native probe failure",
        };
        return Err(format!(
            "IBus roundtrip probe failed: {detail} (stage {code})"
        ));
    }

    let length = committed
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(committed.len());
    String::from_utf8(committed[..length].to_vec())
        .map_err(|error| format!("IBus probe returned invalid UTF-8: {error}"))
}

pub fn run() -> Result<(), String> {
    let component_name = CString::new(recommended_component_name())
        .map_err(|_| "IBus component name contains a null byte".to_string())?;
    let engine_name = CString::new(recommended_connection_name())
        .map_err(|_| "IBus engine name contains a null byte".to_string())?;
    let version = CString::new(env!("CARGO_PKG_VERSION"))
        .map_err(|_| "Package version contains a null byte".to_string())?;

    let code = unsafe {
        suzaku_linux_ibus_run(
            component_name.as_ptr(),
            engine_name.as_ptr(),
            version.as_ptr(),
        )
    };
    if code == 0 {
        Ok(())
    } else {
        Err(format!("IBus engine host exited with status {code}"))
    }
}

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
        wait_for_llm: bool,
        complete_word: bool,
        privacy_mode: u32,
        committed: *mut std::os::raw::c_char,
        committed_capacity: usize,
        preedit: *mut std::os::raw::c_char,
        preedit_capacity: usize,
        primary_candidate: *mut std::os::raw::c_char,
        primary_candidate_capacity: usize,
        candidate_count: *mut usize,
        page_size: *mut usize,
        selected_index: *mut usize,
    ) -> std::os::raw::c_int;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxImeProbeReport {
    pub preedit: String,
    pub primary_candidate: String,
    pub candidate_count: usize,
    pub page_size: usize,
    pub selected_index: usize,
    pub committed: String,
}

pub fn probe_roundtrip(seed: &str) -> Result<String, String> {
    probe_roundtrip_report(seed).map(|report| report.committed)
}

pub fn probe_ipc_roundtrip(text: &str) -> Result<String, String> {
    probe_ipc_roundtrip_report(text).map(|report| report.committed)
}

pub fn probe_roundtrip_report(seed: &str) -> Result<LinuxImeProbeReport, String> {
    probe_roundtrip_mode(seed, false, false, false, 0)
}

pub fn probe_ipc_roundtrip_report(text: &str) -> Result<LinuxImeProbeReport, String> {
    probe_roundtrip_mode(text, true, false, false, 0)
}

pub fn probe_llm_roundtrip_report(seed: &str) -> Result<LinuxImeProbeReport, String> {
    probe_roundtrip_mode(seed, false, true, false, 0)
}

/// Exercise Tab selection and Space commit of the best English word completion.
pub fn probe_completion_roundtrip_report(seed: &str) -> Result<LinuxImeProbeReport, String> {
    probe_roundtrip_mode(seed, false, false, true, 0)
}

pub fn probe_private_roundtrip_report(
    seed: &str,
    password: bool,
) -> Result<LinuxImeProbeReport, String> {
    probe_roundtrip_mode(seed, false, false, false, if password { 2 } else { 1 })
}

fn probe_roundtrip_mode(
    seed: &str,
    via_ipc: bool,
    wait_for_llm: bool,
    complete_word: bool,
    privacy_mode: u32,
) -> Result<LinuxImeProbeReport, String> {
    let engine_name = CString::new(recommended_connection_name())
        .map_err(|_| "IBus engine name contains a null byte".to_string())?;
    let seed = CString::new(seed).map_err(|_| "Probe seed contains a null byte".to_string())?;
    let mut committed = vec![0_u8; 4096];
    let mut preedit = vec![0_u8; 4096];
    let mut primary_candidate = vec![0_u8; 4096];
    let mut candidate_count = 0;
    let mut page_size = 0;
    let mut selected_index = 0;
    let code = unsafe {
        suzaku_linux_ibus_probe_roundtrip(
            engine_name.as_ptr(),
            seed.as_ptr(),
            via_ipc,
            wait_for_llm,
            complete_word,
            privacy_mode,
            committed.as_mut_ptr().cast(),
            committed.len(),
            preedit.as_mut_ptr().cast(),
            preedit.len(),
            primary_candidate.as_mut_ptr().cast(),
            primary_candidate.len(),
            &mut candidate_count,
            &mut page_size,
            &mut selected_index,
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
            12 => "the client did not receive visible preedit text",
            13 => "the client did not receive a visible candidate lookup table",
            14 => "the candidate window is not using the compact three-row layout",
            15 => "the candidate window appeared before an input event",
            16 => "preedit or candidates remained visible after committing",
            17 => "no asynchronous AI candidate arrived before the probe deadline",
            18 => "a password field key was consumed or displayed by Suzaku",
            19 => "an AI candidate appeared in a private input context",
            20 => "Tab did not select the first word completion",
            21 => "word completion did not commit the selected word with exactly one space",
            _ => "unknown native probe failure",
        };
        return Err(format!(
            "IBus roundtrip probe failed: {detail} (stage {code})"
        ));
    }

    Ok(LinuxImeProbeReport {
        preedit: decode_probe_text(&preedit, "preedit")?,
        primary_candidate: decode_probe_text(&primary_candidate, "candidate")?,
        candidate_count,
        page_size,
        selected_index,
        committed: decode_probe_text(&committed, "commit")?,
    })
}

fn decode_probe_text(buffer: &[u8], field: &str) -> Result<String, String> {
    let length = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    String::from_utf8(buffer[..length].to_vec())
        .map_err(|error| format!("IBus probe returned invalid UTF-8 for {field}: {error}"))
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

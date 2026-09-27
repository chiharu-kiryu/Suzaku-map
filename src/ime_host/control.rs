//! Two-phase native controls: prepare durable settings without locking the input
//! session, then publish/apply only while the owning native request is still valid.
use super::{HostImeSession, into_raw_c_string, read_optional_utf8, with_shared_host_ime_session};
use crate::ime::settings::{
    ImeSettings, PanelImeSettingsPatch, PreparedSettingsSave, settings_path,
};
use crate::languages::BuiltinLanguage;
use std::{
    ffi::{c_char, c_void},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

pub(super) fn settings_for_command(
    command: &str,
    current: &ImeSettings,
    reload: impl FnOnce() -> Result<ImeSettings, String>,
) -> Result<Option<ImeSettings>, String> {
    let mut settings = current.clone();
    match command {
        "S" => return Ok(None),
        "R" => return reload().map(Some),
        "P0" => settings.llm_enabled = false,
        "P1" => settings.llm_enabled = true,
        patch if patch.starts_with('U') => {
            PanelImeSettingsPatch::from_json(&patch[1..])?.apply(&mut settings)?;
        }
        language if language.starts_with('L') => {
            settings.language =
                BuiltinLanguage::resolve(&language[1..]).ok_or("不支持的输入语言")?;
        }
        _ => return Err("不支持的输入法控制命令".into()),
    }
    Ok(Some(settings))
}

pub(super) fn apply_settings(session: &mut HostImeSession, settings: ImeSettings, reload: bool) {
    // A repeated narrow control still validates/persists, but must preserve the
    // candidate selection, completion undo, Compose and pending prediction.
    if reload
        || settings.language != session.settings.language
        || settings.llm_enabled != session.settings.llm_enabled
        || settings.provider != session.settings.provider
    {
        session.apply_settings(settings);
    } else {
        session.settings = settings;
    }
}

pub(super) fn response(session: &HostImeSession, result: Result<(), String>) -> serde_json::Value {
    serde_json::json!({"ok": result.is_ok(), "error": result.err(),
        "settings": session.settings.to_json(), "prediction": format!("{:?}", session.engine.prediction_status()),
        "prediction_error": session.engine.prediction_error().map(ToString::to_string)})
}

static CONTROL_BUSY: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        CONTROL_BUSY.store(false, Ordering::Release);
    }
}

struct PreparedControl {
    settings: ImeSettings,
    save: PreparedSettingsSave,
}

struct ControlJob {
    expected: ImeSettings,
    reload: bool,
    deadline: Instant,
    result: Option<Receiver<Result<PreparedControl, String>>>,
    error: Option<String>,
    // Worker retains a second reference: canceling a slow job must not allow
    // an unbounded number of replacement threads waiting on the same disk.
    _permit: Option<Arc<Permit>>,
}

impl ControlJob {
    fn start(command: String, timeout_ms: u32) -> Self {
        let expected = with_shared_host_ime_session(|session| session.settings.clone());
        let mut job = Self {
            expected,
            reload: command == "R",
            deadline: Instant::now() + Duration::from_millis(u64::from(timeout_ms.min(1000))),
            result: None,
            error: None,
            _permit: None,
        };
        if CONTROL_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            job.error = Some("输入法设置正在保存，请稍后重试".into());
            return job;
        }
        let permit = Arc::new(Permit);
        job._permit = Some(permit.clone());
        let expected = job.expected.clone();
        let path = settings_path().ok_or_else(|| "无法定位输入法设置目录".to_string());
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("suzaku-settings".into())
            .spawn(move || {
                let _permit = permit;
                let prepared = (|| {
                    let path = path?;
                    let settings =
                        settings_for_command(&command, &expected, || ImeSettings::load_at(&path))?
                            .ok_or("状态查询不需要保存任务")?;
                    let baseline = if command == "R" { &settings } else { &expected };
                    let save = settings.prepare_save_at(&path, Some(baseline))?;
                    Ok(PreparedControl { settings, save })
                })();
                // A canceled client's receiver is gone; dropping the unsent result
                // cleans the staged file without ever replacing the active settings.
                let _ = sender.send(prepared);
            });
        match worker {
            Ok(_) => job.result = Some(receiver),
            Err(error) => job.error = Some(format!("无法启动设置保存任务：{error}")),
        }
        job
    }

    fn poll(&mut self) -> Option<String> {
        let result = if let Some(error) = self.error.take() {
            Err(error)
        } else {
            match self.result.as_ref()?.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => Err("输入法设置保存任务已停止".into()),
            }
        };
        Some(with_shared_host_ime_session(|session| {
            let result = result.and_then(|prepared| {
                if Instant::now() >= self.deadline {
                    return Err("输入法设置请求超时，未应用修改".into());
                }
                if session.settings != self.expected {
                    return Err("输入法设置已更改，请重试（未应用过期修改）".into());
                }
                prepared.save.commit()?;
                apply_settings(session, prepared.settings, self.reload);
                Ok(())
            });
            response(session, result).to_string()
        }))
    }
}

/// Start an owned job; only the native event-loop thread may poll/free its handle.
#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_control_start_utf8(
    raw: *const c_char,
    timeout_ms: u32,
) -> *mut c_void {
    let Some(command) = read_optional_utf8(raw) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(ControlJob::start(command, timeout_ms))).cast()
}

/// # Safety
/// `job` must be a live, exclusively borrowed handle returned by `control_start_utf8`.
/// Poll on the input thread, only while its connection or focus-bound gesture is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn suzaku_host_ime_control_poll_utf8(job: *mut c_void) -> *mut c_char {
    let Some(job) = (unsafe { job.cast::<ControlJob>().as_mut() }) else {
        return std::ptr::null_mut();
    };
    job.poll()
        .map(into_raw_c_string)
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `job` must be null or an owned `control_start_utf8` handle, freed exactly once
/// on the input thread with no simultaneous poll. This never joins a worker.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn suzaku_host_ime_control_free(job: *mut c_void) {
    if !job.is_null() {
        drop(unsafe { Box::from_raw(job.cast::<ControlJob>()) });
    }
}

use crate::ime::settings::ImeSettings;
use crate::ime::{
    CommitOptions, CommitResult, EngineConfig, InputSource, SignalState, Snapshot,
    XRTabletImeEngine,
};
use crate::languages::{BuiltinLanguage, model::HttpModelProvider};
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostImeUpdate {
    pub active: bool,
    pub marked_text: String,
    pub draft_text: String,
    pub selected_index: usize,
    pub candidates: Vec<String>,
    pub committed_text: String,
}

pub struct HostImeSession {
    engine: XRTabletImeEngine,
    active: bool,
    settings: ImeSettings,
    private: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostImeBridgeSnapshot {
    pub active: bool,
    pub marked_text: String,
    pub draft_text: String,
    pub committed_text: String,
    pub candidate_count: usize,
    pub candidate_labels: Vec<String>,
    pub primary_candidate: Option<String>,
    pub selected_index: usize,
}

impl HostImeSession {
    pub fn new(config: EngineConfig) -> Self {
        let settings = ImeSettings {
            language: BuiltinLanguage::resolve(&config.default_language)
                .unwrap_or(BuiltinLanguage::English),
            ..Default::default()
        };
        Self {
            engine: XRTabletImeEngine::new(config),
            active: false,
            settings,
            private: false,
        }
    }

    pub fn activate(&mut self) -> HostImeUpdate {
        self.active = true;
        self.current_update()
    }

    pub fn deactivate(&mut self) -> HostImeUpdate {
        self.active = false;
        self.engine.clear_session_context();
        self.current_update()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn update_signal(&mut self, signal: SignalState) -> HostImeUpdate {
        self.engine.update_signal(signal);
        self.current_update()
    }

    pub fn replace_marked_text(&mut self, text: &str, source: InputSource) -> HostImeUpdate {
        self.engine.set_source(source);
        self.engine.seed(text);
        self.current_update()
    }

    pub fn clear_marked_text(&mut self) -> HostImeUpdate {
        self.engine.set_source(InputSource::OnScreenPanel);
        self.engine.seed("");
        self.current_update()
    }

    pub fn move_selection(&mut self, delta: isize) -> HostImeUpdate {
        self.engine.move_selection(delta);
        self.current_update()
    }

    pub fn select_candidate(&mut self, index: usize) -> HostImeUpdate {
        self.engine.select_candidate(index);
        self.current_update()
    }

    pub fn commit_selected(&mut self, options: CommitOptions) -> (CommitResult, HostImeUpdate) {
        let result = self.engine.commit(options);
        if self.private {
            self.engine.clear_session_context();
        }
        let update = self.current_update();
        (result, update)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.engine.snapshot()
    }

    pub fn apply_settings(&mut self, settings: ImeSettings) {
        if self.settings.language != settings.language
            || self.settings.provider.scope != settings.provider.scope
            || self.settings.provider.endpoint != settings.provider.endpoint
            || self.settings.provider.model != settings.provider.model
            || self.settings.provider.protocol != settings.provider.protocol
        {
            self.engine.clear_session_context();
        }
        self.engine.set_language(settings.language.id());
        let provider = (settings.llm_enabled && !self.private).then(|| {
            Arc::new(HttpModelProvider::new(settings.provider.clone()))
                as Arc<dyn crate::languages::llm::LlmCompletionProvider>
        });
        self.engine.configure_prediction(provider);
        self.settings = settings;
    }

    pub fn set_private(&mut self, private: bool) {
        if self.private != private {
            self.private = private;
            self.engine.clear_session_context();
            self.apply_settings(self.settings.clone());
        }
    }

    pub fn current_update(&self) -> HostImeUpdate {
        let snapshot = self.engine.snapshot();
        HostImeUpdate {
            active: self.active,
            marked_text: snapshot.seed_text.clone(),
            draft_text: snapshot.draft_text.clone(),
            selected_index: snapshot.selected_index,
            candidates: snapshot.candidate_labels.clone(),
            committed_text: snapshot.committed_text.clone(),
        }
    }
}

static HOST_IME_BRIDGE_SESSION: OnceLock<Mutex<HostImeSession>> = OnceLock::new();
static HOST_IME_BRIDGE_LAST_COMMIT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn test_session_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn shared_host_ime_session() -> &'static Mutex<HostImeSession> {
    HOST_IME_BRIDGE_SESSION.get_or_init(|| {
        let mut session = HostImeSession::new(EngineConfig::default());
        let settings = ImeSettings::load().unwrap_or_else(|error| {
            eprintln!("Suzaku: {error}; using safe local defaults");
            ImeSettings::default()
        });
        session.apply_settings(settings);
        Mutex::new(session)
    })
}

fn shared_last_commit() -> &'static Mutex<Option<String>> {
    HOST_IME_BRIDGE_LAST_COMMIT.get_or_init(|| Mutex::new(None))
}

fn with_shared_host_ime_session<T>(f: impl FnOnce(&mut HostImeSession) -> T) -> T {
    let mutex = shared_host_ime_session();
    let mut guard = mutex.lock().expect("shared host ime session lock poisoned");
    f(&mut guard)
}

pub fn host_bridge_snapshot() -> HostImeBridgeSnapshot {
    with_shared_host_ime_session(|session| {
        let update = session.current_update();
        HostImeBridgeSnapshot {
            active: update.active,
            marked_text: update.marked_text,
            draft_text: update.draft_text,
            committed_text: update.committed_text,
            candidate_count: update.candidates.len(),
            primary_candidate: update.candidates.first().cloned(),
            candidate_labels: update.candidates,
            selected_index: update.selected_index,
        }
    })
}

pub fn reset_host_bridge_session() {
    with_shared_host_ime_session(|session| {
        *session = HostImeSession::new(EngineConfig::default());
    });
    *shared_last_commit()
        .lock()
        .expect("shared host ime last commit lock poisoned") = None;
}

pub fn host_bridge_activate() -> bool {
    with_shared_host_ime_session(|session| {
        session.activate();
        true
    })
}

pub fn host_bridge_deactivate() {
    with_shared_host_ime_session(|session| {
        session.deactivate();
    });
}

pub fn host_bridge_replace_marked_text(text: &str, source: InputSource) -> bool {
    with_shared_host_ime_session(|session| {
        session.replace_marked_text(text, source);
        true
    })
}

pub fn host_bridge_clear_marked_text() {
    with_shared_host_ime_session(|session| {
        session.clear_marked_text();
    });
}

pub fn host_bridge_move_selection(delta: isize) {
    with_shared_host_ime_session(|session| {
        session.move_selection(delta);
    });
}

pub fn host_bridge_select_candidate(index: usize) {
    with_shared_host_ime_session(|session| {
        session.select_candidate(index);
    });
}

pub fn host_bridge_commit_selected(force: bool) -> bool {
    with_shared_host_ime_session(|session| {
        let previous = session.snapshot().committed_text;
        let (result, _update) = session.commit_selected(CommitOptions { force });
        if result.ok {
            let committed_chunk = result
                .text
                .as_deref()
                .map(|text| extract_new_commit_chunk(&previous, text))
                .unwrap_or_default();
            *shared_last_commit()
                .lock()
                .expect("shared host ime last commit lock poisoned") = Some(committed_chunk);
        }
        result.ok
    })
}

pub fn host_bridge_take_last_committed_text() -> Option<String> {
    shared_last_commit()
        .lock()
        .expect("shared host ime last commit lock poisoned")
        .take()
}

fn read_optional_utf8(raw: *const std::os::raw::c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }

    let value = unsafe { CStr::from_ptr(raw) }.to_string_lossy().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn bridge_display_text(session: &HostImeSession) -> String {
    let update = session.current_update();
    if !update.draft_text.is_empty() {
        update.draft_text
    } else {
        update.marked_text
    }
}

fn extract_new_commit_chunk(previous: &str, committed: &str) -> String {
    if previous.is_empty() {
        return committed.to_string();
    }

    if let Some(rest) = committed.strip_prefix(previous) {
        return rest.trim_start().to_string();
    }

    committed.to_string()
}

fn into_raw_c_string(value: String) -> *mut std::os::raw::c_char {
    CString::new(value)
        .expect("host ime bridge string must not contain interior nulls")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_companion_snapshot_utf8(
    host: *const std::os::raw::c_char,
    context: u64,
    revision: u64,
    focused: bool,
    private: bool,
) -> *mut std::os::raw::c_char {
    use crate::ime::companion::{MAX_FRAME_BYTES, NativeCandidate, NativeComposition};
    let Some(host) = read_optional_utf8(host) else {
        return std::ptr::null_mut();
    };
    let mut frame = with_shared_host_ime_session(|session| {
        let snapshot = session.engine.snapshot();
        let visible = focused
            && session.active
            && !private
            && !session.private
            && !snapshot.seed_text.is_empty();
        NativeComposition {
            host,
            context,
            revision,
            focused,
            private: private || session.private,
            language: snapshot.active_language,
            seed: if visible {
                snapshot.seed_text
            } else {
                String::new()
            },
            selected: if visible { snapshot.selected_index } else { 0 },
            candidates: if visible {
                session
                    .engine
                    .candidates()
                    .iter()
                    .map(|c| NativeCandidate {
                        text: c.text.clone(),
                        label: c.label.clone(),
                        kind: c.kind,
                        source: c.source,
                        weight: c.score.clamp(0.0, 100.0).round() as u8,
                    })
                    .collect()
            } else {
                Vec::new()
            },
        }
    });
    let mut raw = frame.to_json().to_string();
    // Fail closed rather than truncating a composition into a different candidate.
    if raw.len() >= MAX_FRAME_BYTES || NativeComposition::parse(raw.as_bytes()).is_err() {
        frame.private = true;
        frame.seed.clear();
        frame.candidates.clear();
        frame.selected = 0;
        raw = frame.to_json().to_string();
    }
    into_raw_c_string(raw + "\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_activate() -> bool {
    host_bridge_activate()
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_deactivate() {
    host_bridge_deactivate();
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_poll_prediction() -> bool {
    with_shared_host_ime_session(|session| {
        session.active && !session.private && session.engine.poll_prediction()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_prediction_pending() -> bool {
    with_shared_host_ime_session(|session| {
        session.active && !session.private && session.engine.prediction_pending()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_set_private(private: bool) {
    with_shared_host_ime_session(|session| session.set_private(private));
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_language_kind() -> u32 {
    with_shared_host_ime_session(|session| match session.settings.language {
        BuiltinLanguage::ChineseSimplified => 1,
        BuiltinLanguage::English => 2,
        BuiltinLanguage::Japanese => 3,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_control_utf8(
    raw: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    let Some(command) = read_optional_utf8(raw) else {
        return std::ptr::null_mut();
    };
    let value = with_shared_host_ime_session(|session| {
        let mut settings = session.settings.clone();
        let change = match command.as_str() {
            "S" => Ok(false),
            "R" => match ImeSettings::load() {
                Ok(loaded) => {
                    settings = loaded;
                    Ok(true)
                }
                Err(error) => Err(error),
            },
            "P0" => {
                settings.llm_enabled = false;
                Ok(true)
            }
            "P1" => {
                settings.llm_enabled = true;
                Ok(true)
            }
            patch if patch.starts_with('U') => {
                crate::ime::settings::PredictionSettingsPatch::from_json(&patch[1..])
                    .and_then(|patch| patch.apply(&mut settings))
                    .map(|()| true)
            }
            language if language.starts_with('L') => match BuiltinLanguage::resolve(&language[1..])
            {
                Some(language) => {
                    settings.language = language;
                    Ok(true)
                }
                None => Err("不支持的输入语言".to_string()),
            },
            _ => Err("不支持的输入法控制命令".to_string()),
        };
        let result = change.and_then(|changed| {
            if changed {
                settings.save()?;
                session.apply_settings(settings);
            }
            Ok(())
        });
        serde_json::json!({"ok": result.is_ok(), "error": result.err(),
            "settings": session.settings.to_json(), "prediction": format!("{:?}", session.engine.prediction_status()),
            "prediction_error": session.engine.prediction_error().map(ToString::to_string)})
    });
    into_raw_c_string(value.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_replace_marked_text_utf8(
    raw_text: *const std::os::raw::c_char,
) -> bool {
    let Some(text) = read_optional_utf8(raw_text) else {
        return false;
    };

    host_bridge_replace_marked_text(&text, InputSource::HardwareKeyboard)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_clear_marked_text() {
    host_bridge_clear_marked_text();
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_move_selection(delta: isize) {
    host_bridge_move_selection(delta);
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_select_candidate(index: usize) {
    host_bridge_select_candidate(index);
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_commit_selected(force: bool) -> bool {
    host_bridge_commit_selected(force)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_candidate_count() -> usize {
    host_bridge_snapshot().candidate_count
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_selected_index() -> usize {
    host_bridge_snapshot().selected_index
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_enable_ibus_candidates() {
    with_shared_host_ime_session(|session| session.engine.enable_ibus_candidate_mix());
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_completion_text_utf8(
    explicit_only: bool,
) -> *mut std::os::raw::c_char {
    with_shared_host_ime_session(|session| {
        if !session.active {
            return std::ptr::null_mut();
        }
        session
            .engine
            .selected_completion_text(explicit_only)
            .map(|text| into_raw_c_string(text.to_owned()))
            .unwrap_or(std::ptr::null_mut())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_ibus_candidate_label_utf8(
    index: usize,
) -> *mut std::os::raw::c_char {
    with_shared_host_ime_session(|session| {
        session
            .engine
            .candidates()
            .get(index)
            .map(|c| {
                into_raw_c_string(crate::ime::candidate_mix::display_label(
                    &c.text,
                    c.kind,
                    c.source,
                    c.score.clamp(0.0, 100.0).round() as u8,
                ))
            })
            .unwrap_or(std::ptr::null_mut())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_candidate_label_utf8(index: usize) -> *mut std::os::raw::c_char {
    let snapshot = host_bridge_snapshot();
    match snapshot.candidate_labels.get(index) {
        Some(label) if !label.is_empty() => into_raw_c_string(label.clone()),
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_primary_candidate_utf8() -> *mut std::os::raw::c_char {
    let snapshot = host_bridge_snapshot();
    match snapshot.primary_candidate {
        Some(candidate) if !candidate.is_empty() => into_raw_c_string(candidate),
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_display_text_utf8() -> *mut std::os::raw::c_char {
    let value = with_shared_host_ime_session(|session| bridge_display_text(session));
    if value.is_empty() {
        std::ptr::null_mut()
    } else {
        into_raw_c_string(value)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_take_last_committed_text_utf8() -> *mut std::os::raw::c_char {
    match host_bridge_take_last_committed_text() {
        Some(text) if !text.is_empty() => into_raw_c_string(text),
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_ime_free_utf8(raw: *mut std::os::raw::c_char) {
    if raw.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(raw);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HostImeSession, host_bridge_commit_selected, host_bridge_replace_marked_text,
        host_bridge_snapshot, host_bridge_take_last_committed_text, reset_host_bridge_session,
        suzaku_host_ime_activate, suzaku_host_ime_candidate_count,
        suzaku_host_ime_candidate_label_utf8, suzaku_host_ime_clear_marked_text,
        suzaku_host_ime_commit_selected, suzaku_host_ime_deactivate,
        suzaku_host_ime_display_text_utf8, suzaku_host_ime_free_utf8,
        suzaku_host_ime_move_selection, suzaku_host_ime_primary_candidate_utf8,
        suzaku_host_ime_replace_marked_text_utf8, suzaku_host_ime_select_candidate,
        suzaku_host_ime_selected_index, suzaku_host_ime_take_last_committed_text_utf8,
    };
    use crate::ime::{CommitOptions, EngineConfig, InputSource, SignalState};
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    fn host_bridge_test_lock() -> std::sync::MutexGuard<'static, ()> {
        super::test_session_lock()
    }

    #[test]
    fn host_session_tracks_marked_text_and_candidates() {
        let _guard = host_bridge_test_lock();
        let mut session = HostImeSession::new(EngineConfig::default());
        session.activate();
        let update = session.replace_marked_text("ni hao", InputSource::HardwareKeyboard);

        assert!(update.active);
        assert_eq!(update.marked_text, "ni hao");
        assert!(!update.candidates.is_empty());
    }

    #[test]
    fn authorized_cloud_provider_stays_disabled_in_private_fields() {
        use crate::languages::model::{ModelProviderConfig, ModelScope};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut session = HostImeSession::new(EngineConfig::default());
        session.set_private(true);
        let started = std::time::Instant::now();
        session.apply_settings(crate::ime::settings::ImeSettings {
            llm_enabled: true,
            provider: ModelProviderConfig {
                scope: ModelScope::Cloud,
                cloud_consent: true,
                endpoint: format!(
                    "https://{}/v1/chat/completions",
                    listener.local_addr().unwrap()
                ),
                model: "arbitrary-family".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        session.activate();
        let update = session.replace_marked_text("hel", InputSource::HardwareKeyboard);
        assert!(!update.candidates.is_empty());
        assert_eq!(
            session.engine.prediction_status(),
            crate::ime::PredictionStatus::Disabled
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(300));
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn private_sessions_disable_predictions_and_never_retain_commit_context() {
        let mut session = HostImeSession::new(EngineConfig::default());
        session.apply_settings(crate::ime::settings::ImeSettings {
            language: crate::languages::BuiltinLanguage::ChineseSimplified,
            llm_enabled: true,
            ..Default::default()
        });
        session.activate();
        session.set_private(true);
        session.replace_marked_text("nihao", InputSource::HardwareKeyboard);
        assert_eq!(
            session.engine.prediction_status(),
            crate::ime::PredictionStatus::Disabled
        );
        let (result, update) = session.commit_selected(CommitOptions { force: true });
        assert_eq!(result.text.as_deref(), Some("你好"));
        assert!(update.committed_text.is_empty());
        session.set_private(false);
        assert_eq!(
            session.engine.prediction_status(),
            crate::ime::PredictionStatus::Idle
        );
    }

    #[test]
    fn deactivation_clears_context_and_pending_composition() {
        let mut session = HostImeSession::new(EngineConfig::default());
        session.activate();
        session.replace_marked_text("hello", InputSource::HardwareKeyboard);
        session.commit_selected(CommitOptions { force: true });
        session.replace_marked_text("world", InputSource::HardwareKeyboard);
        let update = session.deactivate();
        assert!(update.marked_text.is_empty());
        assert!(update.committed_text.is_empty());
        assert!(update.candidates.is_empty());
    }

    #[test]
    fn host_session_commit_moves_text_into_committed_state() {
        let _guard = host_bridge_test_lock();
        let mut session = HostImeSession::new(EngineConfig::default());
        session.activate();
        session.update_signal(SignalState {
            pointer_precision: 0.9,
            gaze_stability: 0.9,
            host_intent_weight: 0.9,
            source_confidence: 0.9,
        });
        session.replace_marked_text("tablet ime", InputSource::HardwareKeyboard);

        let (result, update) = session.commit_selected(CommitOptions { force: true });
        assert!(result.ok);
        assert!(update.marked_text.is_empty());
        assert!(!update.committed_text.is_empty());
    }

    #[test]
    fn host_bridge_snapshot_starts_clean() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let snapshot = host_bridge_snapshot();
        assert!(!snapshot.active);
        assert!(snapshot.marked_text.is_empty());
        assert_eq!(snapshot.candidate_count, 0);
        assert!(snapshot.candidate_labels.is_empty());
    }

    #[test]
    fn extract_new_commit_chunk_prefers_incremental_delta() {
        let _guard = host_bridge_test_lock();
        assert_eq!(
            super::extract_new_commit_chunk("hello", "hello world"),
            "world"
        );
        assert_eq!(
            super::extract_new_commit_chunk("", "hello world"),
            "hello world"
        );
        assert_eq!(
            super::extract_new_commit_chunk("previous", "unrelated text"),
            "unrelated text"
        );
    }

    #[test]
    fn host_bridge_snapshot_starts_with_no_committed_text() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(host_bridge_take_last_committed_text().is_none());
    }

    #[test]
    fn host_bridge_commit_selected_is_noop_until_candidates_exist() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(!host_bridge_commit_selected(false));
        assert!(host_bridge_take_last_committed_text().is_none());
    }

    #[test]
    fn host_bridge_c_api_rejects_null_marked_text_pointer() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(!suzaku_host_ime_replace_marked_text_utf8(std::ptr::null()));
    }

    #[test]
    fn host_bridge_c_api_replaces_marked_text_and_reads_display_text() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_activate());
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));

        let snapshot = host_bridge_snapshot();
        assert!(snapshot.active);
        assert!(snapshot.candidate_count > 0);
        assert_eq!(suzaku_host_ime_candidate_count(), snapshot.candidate_count);

        let raw = suzaku_host_ime_display_text_utf8();
        assert!(!raw.is_null());
        let display = c_string_to_owned(raw);
        assert!(display.unwrap().contains("ni"));
    }

    #[test]
    fn host_bridge_c_api_handles_candidate_queries() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));
        let _ = suzaku_host_ime_activate();

        let first = c_string_to_owned(suzaku_host_ime_candidate_label_utf8(0));
        let missing = suzaku_host_ime_candidate_label_utf8(9999);
        let primary = c_string_to_owned(suzaku_host_ime_primary_candidate_utf8());

        assert!(first.is_some());
        assert!(primary.is_some());
        assert!(missing.is_null());
    }

    #[test]
    fn host_bridge_c_api_rejects_empty_marked_text() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("").expect("empty text");

        assert!(!suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));
        let snapshot = host_bridge_snapshot();
        assert!(snapshot.marked_text.is_empty());
        assert_eq!(snapshot.candidate_count, 0);
        assert!(suzaku_host_ime_candidate_label_utf8(0).is_null());
        assert!(suzaku_host_ime_primary_candidate_utf8().is_null());
    }

    #[test]
    fn host_bridge_c_api_primary_candidate_is_null_before_candidates_exist() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(suzaku_host_ime_primary_candidate_utf8().is_null());
    }

    #[test]
    fn host_bridge_c_api_movement_clamps_selection_index() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        assert!(suzaku_host_ime_activate());
        let text = CString::new("ni hao").expect("seed text");
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));

        let count = host_bridge_snapshot().candidate_count;
        assert!(count > 0);

        suzaku_host_ime_move_selection(99);
        assert_eq!(host_bridge_snapshot().selected_index, count - 1);

        suzaku_host_ime_move_selection(-99);
        assert_eq!(host_bridge_snapshot().selected_index, 0);
    }

    #[test]
    fn host_bridge_c_api_select_candidate_ignores_out_of_range_index() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));

        let start_index = host_bridge_snapshot().selected_index;
        suzaku_host_ime_select_candidate(usize::MAX);
        assert_eq!(host_bridge_snapshot().selected_index, start_index);
    }

    #[test]
    fn host_bridge_c_api_returns_null_display_text_until_seeded() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(suzaku_host_ime_display_text_utf8().is_null());
        let text = CString::new("ni hao").expect("seed text");
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));

        let display = c_string_to_owned(suzaku_host_ime_display_text_utf8());
        assert!(display.as_deref().is_some_and(|value| !value.is_empty()));
    }

    #[test]
    fn host_bridge_c_api_commits_with_force_and_reports_last_chunk() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_activate());
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));

        let committed = suzaku_host_ime_commit_selected(true);
        assert!(committed);
        let last_commit = c_string_to_owned(suzaku_host_ime_take_last_committed_text_utf8());
        assert!(!last_commit.as_deref().unwrap_or_default().is_empty());
    }

    #[test]
    fn host_bridge_c_api_take_last_committed_text_utf8_is_one_shot() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_activate());
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));
        assert!(suzaku_host_ime_commit_selected(true));

        let first = c_string_to_owned(suzaku_host_ime_take_last_committed_text_utf8());
        assert!(first.as_deref().is_some_and(|value| !value.is_empty()));

        let second = suzaku_host_ime_take_last_committed_text_utf8();
        assert!(second.is_null());
    }

    #[test]
    fn host_bridge_c_api_clears_marked_text_and_keeps_snapshot_stable() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));
        assert!(!host_bridge_snapshot().marked_text.is_empty());
        suzaku_host_ime_clear_marked_text();
        assert!(host_bridge_snapshot().marked_text.is_empty());
    }

    #[test]
    fn host_bridge_c_api_tracks_selection_and_free_pointer_api() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        let raw_ptr = CString::new("ni hao").expect("seed text").into_raw();
        assert!(suzaku_host_ime_replace_marked_text_utf8(raw_ptr));
        let selected_before = suzaku_host_ime_selected_index();
        assert_eq!(selected_before, 0);
        // free manually taken from Raw to prove host helper accepts normal C pointers
        suzaku_host_ime_free_utf8(raw_ptr);
    }

    #[test]
    fn host_bridge_c_api_free_utf8_accepts_null() {
        suzaku_host_ime_free_utf8(std::ptr::null_mut());
    }

    #[test]
    fn read_optional_utf8_treats_embedded_nul_and_empty_inputs_as_expected() {
        let with_prefix = b"ni hao\0ignored\0".as_ptr() as *const c_char;
        let blank = b"\0".as_ptr() as *const c_char;

        assert_eq!(
            super::read_optional_utf8(with_prefix),
            Some("ni hao".to_string())
        );
        assert_eq!(super::read_optional_utf8(blank), None);
    }

    #[test]
    fn host_bridge_replace_marked_text_tracks_input_source() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();

        assert!(host_bridge_replace_marked_text(
            "ni hao",
            crate::ime::InputSource::OnScreenPanel
        ));
        assert_eq!(host_bridge_snapshot().marked_text, "ni hao");
    }

    #[test]
    fn host_bridge_c_api_deactivate_clears_active_state() {
        let _guard = host_bridge_test_lock();
        reset_host_bridge_session();
        let text = CString::new("ni hao").expect("seed text");

        assert!(suzaku_host_ime_activate());
        assert!(suzaku_host_ime_replace_marked_text_utf8(text.as_ptr()));
        assert!(host_bridge_snapshot().active);

        suzaku_host_ime_deactivate();
        assert!(!host_bridge_snapshot().active);
    }

    fn c_string_to_owned(raw: *mut c_char) -> Option<String> {
        if raw.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() };
        unsafe {
            let _ = CString::from_raw(raw);
        }
        Some(text)
    }
}

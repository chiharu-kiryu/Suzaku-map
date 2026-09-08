//! Native composition is a view, not a second candidate engine. All transport
//! runs off the UI thread and only the latest snapshot is queued for rendering.
use super::{PanelState, PanelUserEvent};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::JoinHandle;
use suzaku_map::{
    ime::{
        Mode, Snapshot,
        companion::{NativeComposition, NativeOperation},
    },
    panel_support::composition_candidate_previews,
};
use winit::event_loop::EventLoopProxy;

#[derive(Default)]
pub(super) struct NativeView {
    pub frame: Option<NativeComposition>,
    pub showing: bool,
    draft: Option<(String, usize, bool)>,
    pub sender: Option<SyncSender<ActionRequest>>,
    pending: Option<(String, u64)>,
}

pub(super) struct ActionRequest {
    command: String,
    host: String,
    revision: u64,
}

#[derive(Default)]
struct Mailbox {
    update: Option<Option<NativeComposition>>,
}

pub(super) struct NativeSync {
    mailbox: Arc<Mutex<Mailbox>>,
    pub sender: SyncSender<ActionRequest>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl NativeSync {
    pub fn take_update(&self) -> Option<Option<NativeComposition>> {
        self.mailbox.lock().unwrap().update.take()
    }
}

impl Drop for NativeSync {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for thread in self.threads.drain(..) {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

pub(super) fn start(proxy: EventLoopProxy<PanelUserEvent>) -> Option<NativeSync> {
    #[cfg(target_os = "linux")]
    {
        use std::time::Duration;
        use suzaku_map::platform::linux_ime_sync::{Subscription, send_action, socket_path};
        let path = socket_path()?;
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::sync_channel::<ActionRequest>(1);
        let reader_mailbox = mailbox.clone();
        let reader_stop = stop.clone();
        let reader_proxy = proxy.clone();
        let reader_path = path.clone();
        let reader = std::thread::Builder::new()
            .name("suzaku-native-updates".into())
            .spawn(move || {
                let publish = |frame| {
                    let mut slot = reader_mailbox.lock().unwrap();
                    let wake = slot.update.is_none();
                    slot.update = Some(frame);
                    drop(slot);
                    if wake {
                        let _ = reader_proxy.send_event(PanelUserEvent::NativeCompositionReady);
                    }
                };
                let mut backoff = Duration::from_millis(250);
                while !reader_stop.load(Ordering::Acquire) {
                    if let Ok(mut subscription) = Subscription::connect(&reader_path) {
                        let mut received = false;
                        while !reader_stop.load(Ordering::Acquire) {
                            match subscription.next_frame() {
                                Ok(Some(frame)) => {
                                    received = true;
                                    backoff = Duration::from_millis(250);
                                    publish(Some(frame));
                                }
                                Ok(None) => {}
                                Err(_) => break,
                            }
                        }
                        if received {
                            publish(None);
                        }
                    }
                    if !reader_stop.load(Ordering::Acquire) {
                        std::thread::park_timeout(backoff);
                    }
                    backoff = (backoff * 2).min(Duration::from_secs(2));
                }
            })
            .ok()?;
        let action_stop = stop.clone();
        let writer = std::thread::Builder::new()
            .name("suzaku-native-actions".into())
            .spawn(move || {
                while !action_stop.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(action) => {
                            if action_stop.load(Ordering::Acquire) {
                                break;
                            }
                            let result = send_action(&path, &action.command);
                            let _ = proxy.send_event(PanelUserEvent::NativeActionFinished {
                                host: action.host,
                                revision: action.revision,
                                result,
                            });
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => break,
                    }
                }
            });
        match writer {
            Ok(writer) => Some(NativeSync {
                mailbox,
                sender,
                stop,
                threads: vec![reader, writer],
            }),
            Err(_) => {
                stop.store(true, Ordering::Release);
                reader.thread().unpark();
                let _ = reader.join();
                None
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = proxy;
        None
    }
}

impl PanelState {
    pub(super) fn view_snapshot(&self) -> Snapshot {
        if self.native.showing {
            if let Some(frame) = &self.native.frame {
                return frame.snapshot();
            }
            let mut snapshot = self.engine.snapshot();
            snapshot.mode = Mode::Idle;
            snapshot.seed_text.clear();
            snapshot.candidate_labels.clear();
            snapshot.draft_text.clear();
            snapshot.committed_text.clear();
            snapshot.selected_index = 0;
            return snapshot;
        }
        self.engine.snapshot()
    }

    pub(super) fn receive_native_frame(&mut self, frame: Option<NativeComposition>) {
        if let (Some(previous), Some(next)) = (&self.native.frame, &frame) {
            if previous.host == next.host && previous.revision >= next.revision {
                return;
            }
        }
        let visible = frame.as_ref().is_some_and(NativeComposition::visible);
        self.native.frame = frame;
        // Our own text IME context is not an external app to mirror back into itself.
        if self.is_focused || self.chrome.settings_open {
            if self.native.showing && !visible {
                self.refresh_native_view();
                self.window.request_redraw();
            }
            return;
        }
        if !self.native.showing && !visible {
            return;
        }
        if !self.native.showing {
            self.native.draft = Some((
                self.chrome.seed_text.clone(),
                self.chrome.caret_index,
                self.chrome.input_modes_expanded,
            ));
            self.native.showing = true;
            self.engine.configure_prediction(None);
            self.chrome.input_modes_expanded = false;
        }
        // A press begun on older candidates must not select their replacement.
        if matches!(
            self.interaction.pressed_interaction,
            Some(
                suzaku_map::ime::gpu::InteractionKind::Candidate(_)
                    | suzaku_map::ime::gpu::InteractionKind::SelectNextToken(_)
                    | suzaku_map::ime::gpu::InteractionKind::VirtualKeyboardKey(_)
            )
        ) {
            self.clear_pressed_interaction();
            self.interaction.touch_tap_pending = false;
        }
        self.refresh_native_view();
        self.window.request_redraw();
    }

    pub(super) fn refresh_native_view(&mut self) {
        let snapshot = self.view_snapshot();
        let candidates = self
            .native
            .frame
            .as_ref()
            .map(|f| f.engine_candidates())
            .unwrap_or_default();
        let previews = composition_candidate_previews(
            &snapshot.seed_text,
            &snapshot.active_language,
            &candidates,
            6,
            4,
        );
        self.chrome.set_seed_text(snapshot.seed_text);
        self.chrome.move_caret_to_end();
        self.chrome.blur_input();
        self.chrome.composed_tokens.clear();
        self.chrome.next_token_candidates = previews
            .next_tokens
            .iter()
            .map(|e| e.label.clone())
            .collect();
        self.next_token_completions = previews.next_tokens;
        (
            self.chrome.sentence_candidate_source_indices,
            self.chrome.sentence_candidates,
        ) = previews.sentences.into_iter().unzip();
        self.clear_sentence_candidate_scroll();
        self.last_scene = None;
    }

    pub(super) fn leave_native_view(&mut self) {
        if !self.native.showing {
            return;
        }
        self.native.showing = false;
        if let Some((seed, caret, expanded)) = self.native.draft.take() {
            self.chrome.set_seed_text(seed);
            self.chrome.caret_index = caret;
            self.chrome.input_modes_expanded = expanded;
        }
        self.reconfigure_llama_plugin();
        self.last_scene = None;
    }

    /// true means this is a native view, including a rejected/inactive action.
    pub(super) fn native_action(&mut self, operation: NativeOperation) -> bool {
        if !self.native.showing {
            return false;
        }
        #[cfg(target_os = "linux")]
        if !self.is_focused && !self.chrome.settings_open && self.native.pending.is_none() {
            if let Some(frame) = self.native.frame.as_ref() {
                if let (Ok(command), Some(sender)) = (
                    suzaku_map::platform::linux_ime_sync::action_command(frame, &operation),
                    self.native.sender.as_ref(),
                ) {
                    let id = (frame.host.clone(), frame.revision);
                    if sender
                        .try_send(ActionRequest {
                            command,
                            host: id.0.clone(),
                            revision: id.1,
                        })
                        .is_ok()
                    {
                        self.native.pending = Some(id);
                        return true;
                    }
                }
            }
        }
        let _ = operation;
        self.last_commit_feedback =
            Some("Native input changed or is unavailable. Choose a current candidate.".into());
        self.commit_feedback_ticks = 120;
        true
    }

    pub(super) fn native_action_finished(
        &mut self,
        host: String,
        revision: u64,
        result: Result<bool, String>,
    ) {
        if self.native.pending.as_ref() != Some(&(host, revision)) {
            return;
        }
        self.native.pending = None;
        if !matches!(result, Ok(true)) {
            self.last_commit_feedback =
                Some("Input changed; nothing was retried. Choose a current candidate.".into());
            self.commit_feedback_ticks = 120;
            self.window.request_redraw();
        }
    }
}

#[cfg(test)]
pub(super) fn assert_native_view(state: &mut PanelState) {
    use suzaku_map::ime::{companion::NativeCandidate, gpu::InteractionKind};
    state.chrome.set_seed_text("manual draft".into());
    state.refresh_seed();
    let local = state.engine.snapshot();
    let mut frame = NativeComposition {
        host: "00000000-0000-0000-0000-000000000001".into(),
        context: 1,
        revision: 10,
        focused: true,
        private: false,
        language: "en".into(),
        seed: "hel".into(),
        selected: 1,
        candidates: vec![
            NativeCandidate {
                text: "hello".into(),
                label: "hello".into(),
            },
            NativeCandidate {
                text: "hello world".into(),
                label: "hello world · AI".into(),
            },
        ],
    };
    state.receive_native_frame(Some(frame.clone()));
    assert!(state.native.showing);
    assert!(!state.chrome.input_modes_expanded);
    assert_eq!(state.view_snapshot(), frame.snapshot());
    assert_eq!(state.engine.snapshot().seed_text, local.seed_text);
    assert_eq!(state.chrome.seed_text, "hel");
    assert!(!state.chrome.input_focused);
    assert!(!state.chrome.sentence_candidates.is_empty());
    let (sender, receiver) = mpsc::sync_channel(1);
    state.native.sender = Some(sender);
    assert!(state.commit_sentence_candidate(1));
    let request = receiver.try_recv().unwrap();
    assert!(request.command.ends_with(" 10 K1"));
    state.native_action_finished(request.host, request.revision, Ok(false));
    assert_eq!(state.engine.snapshot().committed_text, local.committed_text);
    state.interaction.pressed_interaction = Some(InteractionKind::Candidate(1));
    frame.revision += 1;
    frame.selected = 0;
    state.receive_native_frame(Some(frame.clone()));
    assert!(state.interaction.pressed_interaction.is_none());
    let mut old = frame.clone();
    old.revision -= 1;
    old.seed = "old".into();
    state.receive_native_frame(Some(old));
    assert_eq!(state.chrome.seed_text, "hel");
    frame.revision += 1;
    frame.private = true;
    frame.seed.clear();
    frame.candidates.clear();
    frame.selected = 0;
    state.chrome.settings_open = true;
    state.receive_native_frame(Some(frame));
    assert!(state.chrome.seed_text.is_empty());
    assert!(state.chrome.sentence_candidates.is_empty());
    assert!(state.view_snapshot().candidate_labels.is_empty());
    state.chrome.settings_open = false;
    state.receive_native_frame(None);
    assert!(state.view_snapshot().seed_text.is_empty());
    state.leave_native_view();
    assert_eq!(state.chrome.seed_text, "manual draft");
    assert!(!state.native.showing);
    state.native = Default::default();
    state.chrome.set_seed_text(String::new());
    state.refresh_seed();
    state.last_commit_feedback = None;
    state.commit_feedback_ticks = 0;
}

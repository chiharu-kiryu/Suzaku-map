//! Native composition is a view, not a second candidate engine. All transport
//! runs off the UI thread and only the latest snapshot is queued for rendering.
use super::{PanelState, PanelUserEvent};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::JoinHandle;
use suzaku_map::ime::{
    Mode, Snapshot,
    companion::{NativeComposition, NativeOperation},
    gpu::{InputMode, InteractionKind, NativeCandidatePage, PanelLayoutMode},
};
use winit::event_loop::EventLoopProxy;

#[path = "native_typing.rs"]
mod typing;
use typing::NativeTyping;

#[path = "native_keyboard.rs"]
mod keyboard;
use keyboard::{KeyboardEdit, NativeKeyboardQueue};

#[cfg(all(test, target_os = "linux"))]
#[path = "native_action_audit_test.rs"]
mod action_audit_test;
#[cfg(all(test, target_os = "linux"))]
#[path = "native_sync_test.rs"]
mod tests;
#[cfg(all(test, target_os = "linux"))]
#[path = "tool_chain_audit_test.rs"]
mod tool_chain_audit_test;
#[cfg(all(test, target_os = "linux"))]
pub(super) use tests::assert_insertion_keyboard_followup;
#[cfg(all(test, target_os = "linux"))]
pub(super) use tests::assert_workers_cancel_on_shutdown;
#[cfg(all(test, target_os = "linux"))]
pub(super) use tests::present_test_frame;

#[derive(Default)]
pub(super) struct NativeView {
    pub frame: Option<NativeComposition>,
    pub showing: bool,
    pub(super) presented: Option<super::native_presentation::PresentationTarget>,
    pub(super) occluded: bool,
    draft: Option<(String, usize, bool)>,
    // Only an already displayed public composition may retain its empty docked
    // keyboard. View ownership alone survives focus/disconnect boundaries.
    retained_keyboard_context: Option<(String, u64)>,
    pub sender: Option<SyncSender<ActionRequest>>,
    pending: Option<PendingAction>,
    typing: Option<NativeTyping>,
    // An action ACK does not carry its new revision. Keep the resulting draft
    // as the next keyboard edit's base until an authoritative frame arrives.
    confirmed_action: Option<NativeTyping>,
    keyboard: Option<NativeKeyboardQueue>,
}

struct PendingAction {
    id: (String, u64),
    insertion: Option<NativeInsertion>,
    replacement: Option<NativeTyping>,
}

/// Retain the source until the revision-bound replacement is acknowledged. A
/// snapshot plus generation prevents a late reply from clearing newer work.
pub(super) enum NativeInsertion {
    Translation(super::translation::TranslationInsertion),
    Voice {
        transcript: String,
        generation: u64,
    },
    Handwriting {
        strokes: Vec<Vec<[f32; 2]>>,
        candidates: Vec<String>,
        generation: u64,
    },
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
        use suzaku_map::platform::linux_ime_sync::{
            Subscription, send_action_cancellable, send_keyboard_action_cancellable, socket_path,
        };
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
                    if let Ok(mut subscription) =
                        Subscription::connect_cancellable(&reader_path, &reader_stop)
                    {
                        let mut received = false;
                        while !reader_stop.load(Ordering::Acquire) {
                            match subscription.next_frame_cancellable(&reader_stop) {
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
                            let event = if action.command.starts_with('E') {
                                PanelUserEvent::NativeKeyboardFinished {
                                    host: action.host,
                                    revision: action.revision,
                                    result: send_keyboard_action_cancellable(
                                        &path,
                                        &action.command,
                                        &action_stop,
                                    ),
                                }
                            } else {
                                PanelUserEvent::NativeActionFinished {
                                    host: action.host,
                                    revision: action.revision,
                                    result: send_action_cancellable(
                                        &path,
                                        &action.command,
                                        &action_stop,
                                    ),
                                }
                            };
                            let _ = proxy.send_event(event);
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
    fn native_presentation_target(&self) -> Option<super::native_presentation::PresentationTarget> {
        if self.kind != super::PanelWindowKind::Main
            || !self.runs_without_window_focus
            || !self.native.showing
            || self.native.occluded
            || self.chrome.compact_mode
            || self.has_local_panel_interaction()
            || self.window.is_minimized() == Some(true)
            || self.size.width == 0
            || self.size.height == 0
        {
            return None;
        }
        self.native
            .frame
            .as_ref()
            .filter(|frame| frame.visible())
            .map(|frame| super::native_presentation::PresentationTarget {
                host: frame.host.clone(),
                context: frame.context,
                revision: frame.revision,
            })
    }

    pub(super) fn note_native_frame_presented(&mut self) {
        self.native.presented = self.native_presentation_target();
    }

    pub(super) fn presented_native_target(
        &self,
    ) -> Option<super::native_presentation::PresentationTarget> {
        let current = self.native_presentation_target()?;
        // Keep the displayed revision while the next same-field frame awaits
        // painting. The host rejects stale renewals but retains the short lease,
        // avoiding a system-popup flash on every key. No cross-context carryover.
        self.native
            .presented
            .as_ref()
            .filter(|presented| {
                presented.host == current.host && presented.context == current.context
            })
            .cloned()
    }

    /// Literal edits coalesce, but semantic keys wait for the host's undo/Compose
    /// result. Later keys cannot use a guessed draft while that result is pending.
    pub(super) fn native_keyboard_edit(&mut self, text: Option<&str>) -> bool {
        if !self.native.showing {
            return false;
        }
        if self.native.keyboard.is_some()
            || text == Some(" ")
            || (text.is_none() && self.native.typing.is_none())
        {
            let edit = match text {
                Some(" ") => KeyboardEdit::Continue,
                Some(text) => KeyboardEdit::Text(text.to_owned()),
                None => KeyboardEdit::Backspace,
            };
            self.queue_native_keyboard(edit);
            return true;
        }
        self.native_literal_edit(text);
        true
    }

    fn native_literal_edit(&mut self, text: Option<&str>) {
        if self.native.typing.is_none() {
            let Some(frame) = self
                .native
                .frame
                .as_ref()
                .filter(|f| f.focused && !f.private)
            else {
                self.native_typing_feedback();
                return;
            };
            let mut typing = self
                .native
                .confirmed_action
                .take()
                .unwrap_or_else(|| NativeTyping::new(frame));
            // A pending translation/commit is a different operation, not a keyboard edit.
            typing.blocked |=
                self.native.pending.is_some() || self.is_focused || self.chrome.settings_open;
            self.native.typing = Some(typing);
        }
        if !self.native.typing.as_mut().unwrap().edit(text) {
            self.last_commit_feedback =
                Some("Input exceeds the native draft limit; existing text was retained.".into());
            self.commit_feedback_ticks = 180;
        }
        self.cancel_translation();
        self.flush_native_typing();
        self.refresh_native_view();
    }

    fn queue_native_keyboard(&mut self, edit: KeyboardEdit) {
        if self.native.keyboard.is_none() {
            let Some(frame) = self
                .native
                .frame
                .as_ref()
                .filter(|f| f.focused && !f.private)
            else {
                self.native_typing_feedback();
                return;
            };
            self.native.keyboard = Some(NativeKeyboardQueue::new(frame));
            // Reuse the existing ACK/frame barrier for known actions ahead of
            // this semantic key. A newer, physically edited frame must not make
            // an unconfirmed Adopt look settled and redirect Backspace to it.
            if self.native.typing.is_none() {
                self.native.typing = self.native.confirmed_action.take().or_else(|| {
                    self.native
                        .pending
                        .as_mut()
                        .and_then(|pending| pending.replacement.take())
                });
            }
        }
        let queue = self.native.keyboard.as_mut().unwrap();
        queue.blocked |= self.is_focused || self.chrome.settings_open;
        if !queue.push(edit) {
            queue.blocked = true;
            self.last_commit_feedback = Some("Keyboard queue is full; the last key was not accepted. Recover the retained text in the input field.".into());
            self.commit_feedback_ticks = 180;
        }
        self.cancel_translation();
        self.flush_native_keyboard();
        self.refresh_native_view();
    }

    pub(super) fn suspend_native_keyboard(&mut self) {
        if let Some(queue) = self.native.keyboard.as_mut() {
            queue.blocked = true;
            if let Some(typing) = self.native.typing.as_mut() {
                typing.blocked = true;
            }
            self.native_typing_feedback();
        }
    }

    fn flush_native_keyboard(&mut self) {
        if self.native.pending.is_some()
            || self.native.typing.is_some()
            || self.native.confirmed_action.is_some()
        {
            return;
        }
        let Some(queue) = self.native.keyboard.as_ref() else {
            return;
        };
        if queue.blocked || queue.flight.is_some() {
            return;
        }
        let Some(frame) = self
            .native
            .frame
            .as_ref()
            .filter(|frame| queue.matches(frame))
            .cloned()
        else {
            return;
        };
        if !self.native.showing || self.has_local_panel_interaction() {
            return;
        }
        match queue.front().cloned() {
            None => self.native.keyboard = None,
            Some(KeyboardEdit::Text(text)) => {
                if frame.seed.len().saturating_add(text.len())
                    > suzaku_map::ime::companion::MAX_TEXT_BYTES
                {
                    self.native.keyboard.as_mut().unwrap().blocked = true;
                    self.native_typing_feedback();
                    return;
                }
                self.native.keyboard.as_mut().unwrap().pop();
                if self.native.keyboard.as_ref().unwrap().front().is_none() {
                    self.native.keyboard = None;
                }
                self.native_literal_edit(Some(&text));
            }
            Some(edit) => {
                let operation = match edit {
                    KeyboardEdit::Backspace => NativeOperation::Backspace,
                    KeyboardEdit::Continue => NativeOperation::Continue,
                    KeyboardEdit::Text(_) => unreachable!(),
                };
                #[cfg(target_os = "linux")]
                if let (Ok(command), Some(sender)) = (
                    suzaku_map::platform::linux_ime_sync::action_command(&frame, &operation),
                    self.native.sender.as_ref(),
                ) && sender
                    .try_send(ActionRequest {
                        command,
                        host: frame.host.clone(),
                        revision: frame.revision,
                    })
                    .is_ok()
                {
                    self.native.keyboard.as_mut().unwrap().flight = Some(frame);
                    return;
                }
                let _ = operation;
                self.native.keyboard.as_mut().unwrap().blocked = true;
                self.native_typing_feedback();
            }
        }
    }

    pub(super) fn native_keyboard_finished(
        &mut self,
        host: String,
        revision: u64,
        result: Result<NativeComposition, String>,
    ) {
        let Some(queue) = self.native.keyboard.as_ref() else {
            return;
        };
        let Some(flight) = queue
            .flight
            .as_ref()
            .filter(|flight| flight.host == host && flight.revision == revision)
        else {
            return;
        };
        let result = result.ok().filter(|reply| {
            queue.matches(reply)
                && reply.revision > revision
                && self.native.showing
                && !self.has_local_panel_interaction()
                && self.native.frame.as_ref().is_some_and(|current| {
                    queue.matches(current)
                        && current.revision <= reply.revision
                        && (current.revision != reply.revision
                            || (current.seed == reply.seed
                                && current.selected == reply.selected
                                && current.candidates == reply.candidates))
                })
                && flight.context == reply.context
        });
        let queue = self.native.keyboard.as_mut().unwrap();
        queue.flight = None;
        if let Some(reply) = result.filter(|_| !queue.blocked) {
            queue.pop();
            // Retire the flight before receive_native_frame can flush a follow-up.
            self.receive_native_frame(Some(reply));
            self.flush_native_keyboard();
        } else {
            queue.blocked = true;
            self.native_typing_feedback();
        }
        self.refresh_native_view();
        self.window.request_redraw();
    }

    fn native_typing_feedback(&mut self) {
        self.last_commit_feedback = Some(
            "Keyboard draft is pending or unconfirmed. Click the input field to recover and edit it.".into());
        self.commit_feedback_ticks = 180;
    }

    pub(super) fn take_native_typing_draft(&mut self) -> Option<String> {
        let draft = self
            .native
            .typing
            .take()
            .or_else(|| self.native.confirmed_action.take())
            .map(|typing| typing.draft);
        if let Some(queue) = self.native.keyboard.take() {
            let mut text = draft.unwrap_or_else(|| {
                self.native
                    .frame
                    .as_ref()
                    .filter(|frame| queue.matches(frame))
                    .map(|frame| frame.seed.clone())
                    .unwrap_or_else(|| queue.recovery_seed().to_owned())
            });
            queue.append_recovery_text(&mut text);
            self.last_commit_feedback = Some("Recovered literal text locally; unconfirmed keyboard operations were not replayed.".into());
            self.commit_feedback_ticks = 180;
            Some(text)
        } else {
            draft
        }
    }

    fn flush_native_typing(&mut self) {
        if self
            .native
            .typing
            .as_ref()
            .is_some_and(NativeTyping::settled)
        {
            self.native.typing = None;
            self.flush_native_keyboard();
            return;
        }
        if self
            .native
            .typing
            .as_ref()
            .is_some_and(|typing| typing.blocked)
        {
            self.native_typing_feedback();
            return;
        }
        if self.native.pending.is_some() {
            return;
        }
        let ready = self
            .native
            .typing
            .as_ref()
            .zip(self.native.frame.as_ref())
            .filter(|(typing, frame)| typing.ready(frame))
            .map(|(typing, frame)| (typing.draft.clone(), frame.revision));
        if let Some((text, revision)) = ready {
            if self.enqueue_native_action(NativeOperation::Replace(text), None) {
                self.native.typing.as_mut().unwrap().sent(revision);
            } else {
                self.native.typing.as_mut().unwrap().blocked = true;
                self.native_typing_feedback();
            }
        }
    }

    /// Local editing owns the window even before its asynchronous Focused event.
    /// Native tool tabs may mark the seed focused without acquiring window focus;
    /// those still belong to the external composition, not a local editor.
    pub(super) fn has_local_panel_interaction(&self) -> bool {
        self.is_focused
            || self.chrome.settings_open
            || (!self.native.showing && self.chrome.input_focused)
    }

    /// Nonempty public/queued drafts keep the candidate popup visible. Once a
    /// public native context has been displayed, its expanded bottom keyboard
    /// also stays after commit/deletion so the next draft can start without a
    /// physical key. Empty fields cannot inherit retention from another context
    /// or across focus/privacy/disconnect boundaries.
    pub(super) fn native_composition_visible(&self) -> bool {
        self.native.showing
            && self.native.frame.as_ref().is_some_and(|frame| {
                frame.focused
                    && !frame.private
                    && (!frame.seed.is_empty()
                        || (self.chrome.effective_panel_layout() == PanelLayoutMode::BottomDock
                            && !self.chrome.compact_mode
                            && self.chrome.input_modes_expanded
                            && self.chrome.active_input_mode == InputMode::VirtualKeyboard
                            && self.native.retained_keyboard_context.as_ref().is_some_and(
                                |(host, context)| host == &frame.host && *context == frame.context,
                            ))
                        || self.native.typing.as_ref().is_some_and(|typing| {
                            typing.matches(frame) && !typing.draft.is_empty()
                        })
                        || self
                            .native
                            .keyboard
                            .as_ref()
                            .is_some_and(|queue| queue.matches(frame) && queue.has_text()))
            })
    }

    pub(super) fn view_snapshot(&self) -> Snapshot {
        if self.native.showing {
            if let Some(frame) = &self.native.frame {
                let mut snapshot = frame.snapshot();
                if let Some(typing) = &self.native.typing
                    && typing.matches(frame)
                {
                    snapshot.seed_text = typing.draft.clone();
                    snapshot.draft_text = typing.draft.clone();
                    snapshot.mode = if typing.draft.is_empty() {
                        Mode::Idle
                    } else {
                        Mode::Composing
                    };
                    snapshot.candidate_labels.clear();
                    snapshot.selected_index = 0;
                }
                return snapshot;
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
        if let (Some(previous), Some(next)) = (&self.native.frame, &frame)
            && previous.host == next.host
            && previous.revision >= next.revision
        {
            if let Some(cursor) = super::native_position::geometry_update(previous, next) {
                // Moving the caret is not a new draft. Preserve queued edits,
                // ACK barriers and a still-valid press on the current page.
                self.native.frame.as_mut().unwrap().cursor = cursor;
                self.update_native_position();
                self.window.request_redraw();
            }
            return;
        }
        if self
            .native
            .retained_keyboard_context
            .as_ref()
            .is_some_and(|(host, context)| {
                frame.as_ref().is_none_or(|next| {
                    !next.focused || next.private || &next.host != host || next.context != *context
                })
            })
        {
            self.native.retained_keyboard_context = None;
        }
        if let Some(queue) = self.native.keyboard.as_mut() {
            if frame.as_ref().is_some_and(|frame| !queue.matches(frame)) {
                self.native.keyboard = None;
                self.native_typing_feedback();
            } else if frame.is_none() {
                queue.blocked = true;
            } else if let Some(frame) = frame.as_ref() {
                // Preserve the last authoritative text for recovery even if
                // the reader disconnects and the semantic reply never arrives.
                queue.observe(frame);
            }
        }
        // With no follow-up edit, the latest snapshot wins, even if the mailbox
        // skipped the action's intermediate frame after a physical edit.
        // Once typing starts, NativeTyping instead protects the unsent edits.
        self.native.confirmed_action = None;
        let input_target_unchanged = self.native.showing
            && self
                .native
                .frame
                .as_ref()
                .zip(frame.as_ref())
                .is_some_and(|(old, new)| {
                    old.host == new.host
                        && old.context == new.context
                        && old.language == new.language
                        && old.focused
                        && !old.private
                        && new.focused
                        && !new.private
                });
        // Unlike keyboard/tool edits, translation sends the reviewed source
        // itself. A different draft needs a fresh explicit gesture.
        let translation_source_unchanged = input_target_unchanged
            && self
                .native
                .frame
                .as_ref()
                .zip(frame.as_ref())
                .is_some_and(|(old, new)| old.seed == new.seed);
        if let Some(typing) = &mut self.native.typing {
            if let Some(next) = &frame {
                if typing.matches(next) {
                    typing.observe(next);
                } else {
                    self.native.typing = None;
                }
            } else {
                // Keep a local recovery copy, but never send it on reconnect automatically.
                typing.blocked = true;
            }
        }
        let visible = frame.as_ref().is_some_and(NativeComposition::visible);
        self.native.frame = frame;
        self.pause_voice_capture_if_target_changed();
        // Candidate presses depend on the exact snapshot. Keyboard/tool presses
        // can survive metadata or same-field edits, but never a change of target
        // (including entering native view from local editing). Cancel before the
        // focus/settings early return so an away-and-back update cannot revive
        // a press that began in another field. Retain tool sources for a new click.
        let updates_native_view =
            self.native.showing || (visible && !self.has_local_panel_interaction());
        if updates_native_view
            && (matches!(
                self.interaction.pressed_interaction,
                Some(
                    InteractionKind::Candidate(_)
                        | InteractionKind::SelectNextToken(_)
                        | InteractionKind::NativeCandidatePage(_)
                )
            ) || (!input_target_unchanged
                && matches!(
                    self.interaction.pressed_interaction,
                    Some(
                        InteractionKind::VirtualKeyboardKey(_)
                            | InteractionKind::InsertVoiceTranscript
                            | InteractionKind::UseHandwritingCandidate(_)
                    )
                ))
                || (!translation_source_unchanged
                    && self.interaction.pressed_interaction
                        == Some(InteractionKind::TranslateText)))
        {
            self.clear_pressed_interaction();
            self.interaction.touch_tap_pending = false;
        }
        // Our own text IME context is not an external app to mirror back into itself.
        if self.has_local_panel_interaction() {
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
            self.pause_voice_capture_if_target_changed();
            self.engine.configure_prediction(None);
            if self.chrome.effective_panel_layout() == PanelLayoutMode::FollowCaret {
                self.chrome.input_modes_expanded = false;
            }
        }
        // This must stay after the local-ownership return: receiving a frame
        // while editing our own window is not an external native wakeup.
        if visible && let Some(frame) = &self.native.frame {
            self.native.retained_keyboard_context = Some((frame.host.clone(), frame.context));
        }
        self.flush_native_typing();
        self.flush_native_keyboard();
        self.refresh_native_view();
        self.window.request_redraw();
    }

    pub(super) fn native_candidates_busy(&self) -> bool {
        self.native.pending.is_some()
            || self.native.typing.is_some()
            || self.native.confirmed_action.is_some()
            || self.native.keyboard.is_some()
    }

    pub(super) fn refresh_native_view(&mut self) {
        let snapshot = self.view_snapshot();
        let mut candidates = self
            .native
            .frame
            .as_ref()
            .map(|f| f.engine_candidates())
            .unwrap_or_default();
        // A semantic key has no speculative draft: keep the current host page
        // as a read-only placeholder until its authoritative result arrives.
        // Collapsing it on every B/S request moves the dock and its hit targets
        // even when the resulting composition keeps exactly the same layout.
        // Literal typing still hides candidates for its unconfirmed draft.
        if self.native.typing.is_some() {
            candidates.clear();
        }
        let page = NativeCandidatePage::new(
            snapshot.selected_index,
            candidates.len(),
            self.native_candidates_busy(),
        );
        self.chrome.set_seed_text(snapshot.seed_text);
        self.chrome.move_caret_to_end();
        self.chrome.blur_input();
        self.chrome.composed_tokens.clear();
        // Derived English chips are useful in the standalone editor, but their
        // numbering does not describe IBus's actual six numeric choices.
        self.chrome.next_token_candidates.clear();
        self.next_token_completions.clear();
        (
            self.chrome.sentence_candidate_source_indices,
            self.chrome.sentence_candidates,
        ) = candidates
            .iter()
            .enumerate()
            .skip(page.start)
            .take(NativeCandidatePage::SIZE)
            .map(|(index, candidate)| (index, candidate.label.clone()))
            .unzip();
        self.chrome.native_candidate_page = Some(page);
        self.clear_sentence_candidate_scroll();
        self.last_scene = None;
        self.poll_translation();
    }

    pub(super) fn leave_native_view(&mut self) {
        self.native.retained_keyboard_context = None;
        if !self.native.showing {
            return;
        }
        self.native.showing = false;
        self.chrome.native_candidate_page = None;
        self.pause_voice_capture_if_target_changed();
        self.native.typing = None;
        self.native.confirmed_action = None;
        self.native.keyboard = None;
        if let Some((seed, caret, expanded)) = self.native.draft.take() {
            self.chrome.set_seed_text(seed);
            self.chrome.caret_index = caret;
            // The dock's drawer is an explicit layout choice, not temporary
            // candidate-popup chrome. Do not undo a fold or mode change made
            // while the external native draft owned the view.
            if self.chrome.effective_panel_layout() != PanelLayoutMode::BottomDock {
                self.chrome.input_modes_expanded = expanded;
            }
        }
        self.reconfigure_model_provider();
        self.last_scene = None;
    }

    /// true means this is a native view, including a rejected/inactive action.
    pub(super) fn native_action(&mut self, operation: NativeOperation) -> bool {
        match operation {
            NativeOperation::Backspace => return self.native_keyboard_edit(None),
            NativeOperation::Continue => return self.native_keyboard_edit(Some(" ")),
            _ => {}
        }
        self.native_action_with_source(operation, None)
    }

    pub(super) fn native_insert_text(&mut self, text: &str, source: NativeInsertion) -> bool {
        if !self.native.showing {
            return false;
        }
        let seed = self.view_snapshot().seed_text;
        let (seed, _) =
            suzaku_map::ime::tool_text::insert_tool_text(&seed, seed.chars().count(), text);
        // Do not edit the acknowledged seed or discard the source optimistically.
        self.native_action_with_source(NativeOperation::Replace(seed), Some(source))
    }

    pub(super) fn native_action_with_source(
        &mut self,
        operation: NativeOperation,
        insertion: Option<NativeInsertion>,
    ) -> bool {
        if !self.native.showing {
            return false;
        }
        if self.native.typing.is_some()
            || self.native.confirmed_action.is_some()
            || self.native.keyboard.is_some()
        {
            // Even Clear must wait for confirmation and its authoritative
            // revision. Explicit local recovery remains available in the field.
            self.native_typing_feedback();
            return true;
        }
        self.enqueue_native_action(operation, insertion);
        true
    }

    fn enqueue_native_action(
        &mut self,
        operation: NativeOperation,
        insertion: Option<NativeInsertion>,
    ) -> bool {
        if matches!(
            operation,
            NativeOperation::Backspace | NativeOperation::Continue
        ) {
            return false; // Semantic keys require their authoritative-frame reply.
        }
        #[cfg(target_os = "linux")]
        if !self.is_focused
            && !self.chrome.settings_open
            && self.native.pending.is_none()
            && let Some(frame) = self.native.frame.as_ref()
            && let (Ok(command), Some(sender)) = (
                suzaku_map::platform::linux_ime_sync::action_command(frame, &operation),
                self.native.sender.as_ref(),
            )
        {
            let id = (frame.host.clone(), frame.revision);
            if sender
                .try_send(ActionRequest {
                    command,
                    host: id.0.clone(),
                    revision: id.1,
                })
                .is_ok()
            {
                if insertion.is_some() {
                    self.last_commit_feedback =
                        Some("Inserting; source retained until confirmation.".into());
                    self.commit_feedback_ticks = 120;
                }
                // Screen-keyboard writes already own a tracked flight. Every
                // other action needs the same ACK/frame barrier, not only tools.
                let replacement = self.native.typing.is_none().then(|| {
                    let mut typing = NativeTyping::new(frame);
                    typing.draft = match &operation {
                        NativeOperation::Replace(text) => text.clone(),
                        NativeOperation::Adopt(index) => frame.candidates[*index].text.clone(),
                        NativeOperation::Commit(_) | NativeOperation::Clear => String::new(),
                        // Selection changes the highlighted candidate, not the seed.
                        NativeOperation::Select(_) => frame.seed.clone(),
                        NativeOperation::Backspace | NativeOperation::Continue => unreachable!(),
                    };
                    if let NativeOperation::Select(selected) = &operation {
                        typing.sent_selection(frame.revision, *selected);
                    } else {
                        typing.sent(frame.revision);
                    }
                    typing
                });
                self.native.pending = Some(PendingAction {
                    id,
                    insertion,
                    replacement,
                });
                if let Some(page) = &mut self.chrome.native_candidate_page {
                    page.busy = true;
                }
                self.last_scene = None;
                return true;
            }
        }
        let _ = operation;
        self.last_commit_feedback = Some(
            if insertion.is_some() {
                "Native input is busy or unavailable; source retained. Retry explicitly when ready."
            } else {
                "Native input changed or is unavailable. Choose a current candidate."
            }
            .into(),
        );
        self.commit_feedback_ticks = 120;
        false
    }

    pub(super) fn native_action_finished(
        &mut self,
        host: String,
        revision: u64,
        result: Result<bool, String>,
    ) {
        if self.native.pending.as_ref().map(|pending| &pending.id) != Some(&(host, revision)) {
            return;
        }
        let pending = self.native.pending.take().unwrap();
        if self
            .native
            .typing
            .as_mut()
            .is_some_and(|typing| typing.acknowledge(revision, matches!(result, Ok(true))))
        {
            if matches!(result, Ok(true))
                && let Some(insertion) = pending.insertion
            {
                self.finish_native_insertion(insertion);
            }
            if let (Some(typing), Some(frame)) = (&mut self.native.typing, &self.native.frame)
                && typing.matches(frame)
            {
                typing.observe(frame);
            }
            self.flush_native_typing();
            self.refresh_native_view();
            self.window.request_redraw();
            return;
        }
        if matches!(result, Ok(true)) {
            if let Some(insertion) = pending.insertion {
                self.finish_native_insertion(insertion);
                self.last_commit_feedback = Some("Input added to the native composition.".into());
                self.commit_feedback_ticks = 40;
                self.window.request_redraw();
            }
            if let Some(mut replacement) = pending.replacement
                && self.native.showing
                && self.native.typing.is_none()
                && let Some(frame) = &self.native.frame
                && frame.revision == revision
                && replacement.matches(frame)
            {
                replacement.acknowledge(revision, true);
                // Do not rebase on the old visible seed or invent a new
                // revision. Keyboard edits inherit this acknowledged flight
                // and wait for its matching frame before the next send.
                // Keep newer tool/translation work untouched in the meantime.
                self.native.confirmed_action = Some(replacement);
            }
        } else {
            if let Some(queue) = self.native.keyboard.as_mut() {
                queue.blocked = true;
            }
            self.last_commit_feedback = Some(if pending.insertion.is_some() {
                "Insertion was not confirmed; source retained. Nothing was retried; check the target before retrying."
            } else {
                "Input changed; nothing was retried. Choose a current candidate."
            }.into());
            self.commit_feedback_ticks = 120;
            self.window.request_redraw();
        }
        if self.native.showing {
            self.flush_native_keyboard();
            self.refresh_native_view();
            self.window.request_redraw();
        }
    }

    pub(super) fn native_translation_replacement_visible(&self) -> bool {
        self.native.pending.as_ref().is_some_and(|pending| {
            matches!(
                &pending.insertion,
                Some(NativeInsertion::Translation(insertion))
                    if self.translation_replacement_visible(insertion)
            )
        })
    }

    fn finish_native_insertion(&mut self, insertion: NativeInsertion) {
        use suzaku_map::ime::gpu::InputMode;
        let completed_mode = match insertion {
            NativeInsertion::Translation(insertion) => {
                if !self.finish_translation_insertion(&insertion) {
                    return;
                }
                InputMode::Translation
            }
            NativeInsertion::Voice {
                transcript,
                generation,
            } if self.voice_progress.generation() == generation
                && self.chrome.voice_transcript == transcript =>
            {
                self.clear_voice_transcript();
                InputMode::Dictation
            }
            NativeInsertion::Handwriting {
                strokes,
                candidates,
                generation,
            } if self.handwriting_generation == generation
                && self.chrome.handwriting_strokes == strokes
                && self.chrome.handwriting_candidates == candidates =>
            {
                self.clear_handwriting();
                InputMode::Handwriting
            }
            _ => return,
        };
        // Do not pull the user out of another mode opened while the request ran.
        if self.chrome.active_input_mode == completed_mode {
            self.chrome.active_input_mode = InputMode::VirtualKeyboard;
            self.chrome.input_modes_expanded = false;
        }
        self.last_scene = None;
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
        cursor: None,
        candidates: vec![
            NativeCandidate {
                text: "hello".into(),
                label: "hello".into(),
                ..Default::default()
            },
            NativeCandidate {
                text: "hello world".into(),
                label: "hello world · AI".into(),
                ..Default::default()
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
    assert_native_paging(state);
    assert_bottom_dock_native_lifecycle(state);
    state.chrome.set_seed_text(String::new());
    state.refresh_seed();
    state.last_commit_feedback = None;
    state.commit_feedback_ticks = 0;
}

#[cfg(test)]
fn assert_bottom_dock_native_lifecycle(state: &mut PanelState) {
    let saved_chrome = state.chrome.clone();
    state.chrome.panel_layout_mode = PanelLayoutMode::BottomDock;
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.chrome.input_modes_expanded = true;
    state.chrome.compact_mode = false;
    state.chrome.settings_open = false;
    state.chrome.input_focused = false;
    let mut frame = NativeComposition {
        host: "00000000-0000-0000-0000-000000000004".into(),
        context: 4,
        revision: 1,
        focused: true,
        private: false,
        language: "en".into(),
        seed: String::new(),
        selected: 0,
        cursor: None,
        candidates: Vec::new(),
    };
    state.receive_native_frame(Some(frame.clone()));
    assert!(
        !state.native.showing,
        "an untouched empty field must not wake the dock"
    );
    assert!(!state.native_composition_visible());
    frame.revision += 1;
    frame.seed = "hel".into();
    state.receive_native_frame(Some(frame.clone()));
    assert!(state.native.showing);
    assert!(state.chrome.input_modes_expanded);
    frame.revision += 1;
    frame.seed.clear();
    state.receive_native_frame(Some(frame.clone()));
    assert!(
        state.native_composition_visible(),
        "the next draft needs its screen keyboard"
    );

    for layout in [PanelLayoutMode::FollowCaret, PanelLayoutMode::BottomDock] {
        for expanded in [false, true] {
            for compact in [false, true] {
                for mode in [
                    InputMode::VirtualKeyboard,
                    InputMode::Dictation,
                    InputMode::Handwriting,
                    InputMode::Translation,
                ] {
                    state.chrome.panel_layout_mode = layout;
                    state.chrome.input_modes_expanded = expanded;
                    state.chrome.compact_mode = compact;
                    state.chrome.active_input_mode = mode;
                    assert_eq!(
                        state.native_composition_visible(),
                        layout == PanelLayoutMode::BottomDock
                            && expanded
                            && !compact
                            && mode == InputMode::VirtualKeyboard,
                        "empty native view: {layout:?}/{expanded}/{compact}/{mode:?}"
                    );
                }
            }
        }
    }
    state.chrome.panel_layout_mode = PanelLayoutMode::BottomDock;
    state.chrome.input_modes_expanded = true;
    state.chrome.compact_mode = false;
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    let (sender, receiver) = mpsc::sync_channel(1);
    state.native.sender = Some(sender);
    assert!(state.native_keyboard_edit(Some("x")));
    let request = receiver
        .try_recv()
        .expect("empty native draft accepts a screen key");
    assert!(request.command.ends_with(" 3 Tx"));
    assert_eq!(state.view_snapshot().seed_text, "x");
    frame.revision += 1;
    frame.seed = "x".into();
    state.receive_native_frame(Some(frame.clone()));
    state.native_action_finished(request.host, request.revision, Ok(true));
    assert!(state.native.typing.is_none());
    assert!(state.native.pending.is_none());
    assert!(
        receiver.try_recv().is_err(),
        "the key must not replay after confirmation"
    );

    for boundary in ["context", "host", "private", "focus", "disconnect"] {
        // A real public draft may establish retention again, but an empty
        // snapshot after any boundary may not resurrect the previous wakeup.
        frame.revision += 1;
        frame.seed = "hello".into();
        state.receive_native_frame(Some(frame.clone()));
        frame.revision += 1;
        frame.seed.clear();
        state.receive_native_frame(Some(frame.clone()));
        assert!(state.native_composition_visible());
        let mut next = frame.clone();
        next.revision += 1;
        match boundary {
            "context" => next.context += 1,
            "host" => next.host = "00000000-0000-0000-0000-000000000005".into(),
            "private" => next.private = true,
            "focus" => next.focused = false,
            "disconnect" => {}
            _ => unreachable!(),
        }
        state.receive_native_frame((boundary != "disconnect").then_some(next));
        assert!(
            !state.native_composition_visible(),
            "dock crossed {boundary} boundary"
        );
        assert!(state.native.retained_keyboard_context.is_none());
        frame.revision += 2;
        state.receive_native_frame(Some(frame.clone()));
        assert!(
            !state.native_composition_visible(),
            "empty frame revived a {boundary} wakeup"
        );
    }
    state.receive_native_frame(None);
    assert!(!state.native_composition_visible());
    state.leave_native_view();
    assert!(state.native.retained_keyboard_context.is_none());
    state.native = Default::default();
    state.chrome.input_focused = true;
    frame.revision += 1;
    frame.seed = "local editor owns this update".into();
    state.receive_native_frame(Some(frame.clone()));
    assert!(!state.native.showing);
    assert!(state.native.retained_keyboard_context.is_none());
    state.chrome.input_focused = false;
    frame.revision += 1;
    frame.seed.clear();
    state.receive_native_frame(Some(frame.clone()));
    assert!(
        !state.native_composition_visible(),
        "local editing must not authorize a native empty wakeup"
    );
    state.native = Default::default();

    // Native/local transitions preserve the current dock choice, whereas the
    // ordinary popup still restores its independent pre-native local drawer.
    for layout in [PanelLayoutMode::FollowCaret, PanelLayoutMode::BottomDock] {
        for prior_expanded in [false, true] {
            state.chrome.panel_layout_mode = layout;
            state.chrome.input_modes_expanded = prior_expanded;
            state.chrome.input_focused = false;
            frame.revision += 1;
            frame.focused = true;
            frame.private = false;
            frame.seed = "hello".into();
            state.receive_native_frame(Some(frame.clone()));
            assert!(state.native.showing);
            state.chrome.input_modes_expanded = !prior_expanded;
            state.leave_native_view();
            assert_eq!(
                state.chrome.input_modes_expanded,
                if layout == PanelLayoutMode::BottomDock {
                    !prior_expanded
                } else {
                    prior_expanded
                }
            );
            state.native = Default::default();
        }
    }
    state.chrome = saved_chrome;
    state.last_scene = None;
}

#[cfg(test)]
fn assert_native_paging(state: &mut PanelState) {
    use suzaku_map::ime::companion::NativeCandidate;
    let mut frame = NativeComposition {
        host: "00000000-0000-0000-0000-000000000003".into(),
        context: 3,
        revision: 1,
        focused: true,
        private: false,
        language: "en".into(),
        seed: "candidate".into(),
        selected: 0,
        cursor: None,
        candidates: (0..13)
            .map(|index| NativeCandidate {
                text: format!("candidate {index}"),
                label: format!("candidate {index}"),
                ..Default::default()
            })
            .collect(),
    };
    let (sender, receiver) = mpsc::sync_channel(8);
    state.native.sender = Some(sender);
    for selected in [0, 4, 5, 6, 11, 12, 0] {
        frame.selected = selected;
        frame.revision += 1;
        state.receive_native_frame(Some(frame.clone()));
        let start = selected / 6 * 6;
        assert_eq!(
            state.chrome.sentence_candidate_source_indices,
            (start..(start + 6).min(13)).collect::<Vec<_>>()
        );
        assert!(state.chrome.next_token_candidates.is_empty());
        assert!(state.next_token_completions.is_empty());
        let scene = state.current_scene();
        for index in start..(start + 6).min(13) {
            assert!(
                scene
                    .interactive_targets
                    .iter()
                    .any(|target| target.kind == InteractionKind::Candidate(index))
            );
        }
        assert!(
            !scene
                .interactive_targets
                .iter()
                .any(|target| matches!(target.kind, InteractionKind::SelectNextToken(_)))
        );
    }
    // A frame arriving between down/up invalidates page controls just like cards.
    for touch in [false, true] {
        #[cfg(target_os = "linux")]
        present_test_frame(state);
        let rect = state
            .interaction_rect(InteractionKind::NativeCandidatePage(true))
            .unwrap();
        state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
        state.begin_primary_press(touch);
        assert_eq!(
            state.interaction.pressed_interaction,
            Some(InteractionKind::NativeCandidatePage(true))
        );
        frame.cursor = suzaku_map::ime::companion::NativeCursorRect::new(200, 100, 0, 20);
        state.receive_native_frame(Some(frame.clone()));
        assert_eq!(state.native.frame.as_ref().unwrap().cursor, frame.cursor);
        assert_eq!(
            state.interaction.pressed_interaction,
            Some(InteractionKind::NativeCandidatePage(true)),
            "caret movement alone must preserve a valid candidate press"
        );
        frame.revision += 1;
        state.receive_native_frame(Some(frame.clone()));
        assert!(state.interaction.pressed_interaction.is_none());
        state.complete_primary_release(touch);
        assert!(receiver.try_recv().is_err());
    }
    // Wheel over the input is not paging; a real candidate region is.
    state.cursor_position = Some((0.0, 0.0));
    assert!(!state.scroll_native_candidates(-1.0));
    assert!(receiver.try_recv().is_err());
    for frame_first in [false, true] {
        frame.selected = 0;
        frame.revision += 1;
        state.receive_native_frame(Some(frame.clone()));
        state.change_native_candidate_page(false);
        assert!(receiver.try_recv().is_err());
        let rect = state
            .interaction_rect(InteractionKind::Candidate(5))
            .unwrap();
        state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
        assert!(state.scroll_native_candidates(-1.0));
        let request = receiver.try_recv().unwrap();
        assert!(request.command.ends_with(" N6"));
        assert!(state.chrome.native_candidate_page.unwrap().busy);
        assert!(
            state
                .interaction_rect(InteractionKind::NativeCandidatePage(true))
                .is_none()
        );
        state.change_native_candidate_page(true);
        assert!(receiver.try_recv().is_err());
        frame.selected = 6;
        frame.revision += 1;
        if frame_first {
            state.receive_native_frame(Some(frame.clone()));
        }
        state.native_action_finished(request.host, request.revision, Ok(true));
        if !frame_first {
            assert!(state.chrome.native_candidate_page.unwrap().busy);
            let mut geometry = state.native.frame.clone().unwrap();
            geometry.cursor = suzaku_map::ime::companion::NativeCursorRect::new(400, 100, 0, 20);
            state.receive_native_frame(Some(geometry));
            assert!(state.native.confirmed_action.is_some());
            assert!(state.chrome.native_candidate_page.unwrap().busy);
            state.change_native_candidate_page(true);
            assert!(receiver.try_recv().is_err());
            state.receive_native_frame(Some(frame.clone()));
        }
        assert!(!state.chrome.native_candidate_page.unwrap().busy);
        assert_eq!(
            state.chrome.sentence_candidate_source_indices,
            [6, 7, 8, 9, 10, 11]
        );
    }
    frame.selected = 12;
    frame.revision += 1;
    state.receive_native_frame(Some(frame));
    state.change_native_candidate_page(true);
    assert!(receiver.try_recv().is_err());
    assert!(
        state
            .interaction_rect(InteractionKind::Candidate(13))
            .is_none()
    );
    state.leave_native_view();
    assert!(state.chrome.native_candidate_page.is_none());
    state.native = Default::default();
}

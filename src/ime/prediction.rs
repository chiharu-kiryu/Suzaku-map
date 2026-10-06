//! One worker, one replaceable pending request, and one replaceable result.
//! Editing, committing or changing language invalidates every older response.

use crate::prediction::{
    PredictionCancellation, PredictionCandidate, PredictionError, PredictionInput,
    PredictionProvider, PredictionRequest,
};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PredictionStatus {
    #[default]
    Disabled,
    Idle,
    Pending,
    Ready,
    Unavailable,
}

struct Job {
    revision: u64,
    ready_at: Instant,
    request: PredictionRequest,
}

#[derive(Default)]
struct Mailbox {
    revision: u64,
    stopped: bool,
    pending: Option<Job>,
    inflight: Option<PredictionCancellation>,
    result: Option<(u64, Result<Vec<PredictionCandidate>, PredictionError>)>,
}

pub(crate) struct PredictionWorker {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    debounce: Duration,
}

impl PredictionWorker {
    pub fn new(provider: Arc<dyn PredictionProvider>, debounce: Duration) -> Self {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_mailbox = mailbox.clone();
        let spawned = thread::Builder::new()
            .name("suzaku-prediction".into())
            .spawn(move || {
                let (lock, wake) = &*worker_mailbox;
                loop {
                    let (job, cancellation) = {
                        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                        loop {
                            if state.stopped {
                                return;
                            }
                            if let Some(job) = state.pending.as_ref() {
                                let delay = job.ready_at.saturating_duration_since(Instant::now());
                                if delay.is_zero() {
                                    let cancellation = PredictionCancellation::default();
                                    state.inflight = Some(cancellation.clone());
                                    break (state.pending.take().unwrap(), cancellation);
                                }
                                state = wake
                                    .wait_timeout(state, delay)
                                    .unwrap_or_else(|error| error.into_inner())
                                    .0;
                            } else {
                                state = wake.wait(state).unwrap_or_else(|error| error.into_inner());
                            }
                        }
                    };
                    // No engine/session lock is held while a model is running.
                    let completions =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            cancellation.check()?;
                            job.request.validate()?;
                            let result = provider.predict(&job.request, &cancellation);
                            cancellation.check()?;
                            let mut response = result?;
                            response.validate_for(&job.request)?;
                            // Apply common draft semantics before either host's
                            // display cap. One invalid row must not hide valid siblings.
                            response.candidates.retain(|candidate| {
                                crate::prediction::policy::preserves_input(
                                    &candidate.text,
                                    &job.request.input,
                                )
                            });
                            if response.candidates.is_empty() {
                                return Err(PredictionError::NoCandidates);
                            }
                            Ok(response.candidates)
                        }))
                        .unwrap_or(Err(PredictionError::Unavailable));
                    let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                    state.inflight = None;
                    if !state.stopped && state.revision == job.revision {
                        state.result = Some((job.revision, completions));
                    }
                }
            });
        if spawned.is_err() {
            mailbox.0.lock().unwrap().stopped = true;
        }
        Self { mailbox, debounce }
    }

    pub fn request(&self, input: PredictionInput) -> bool {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
        if state.stopped {
            return false;
        }
        state.revision = state.revision.wrapping_add(1);
        if let Some(inflight) = &state.inflight {
            inflight.cancel();
        }
        state.result = None;
        state.pending = Some(Job {
            revision: state.revision,
            ready_at: Instant::now() + self.debounce,
            request: PredictionRequest::new(state.revision, input),
        });
        wake.notify_one();
        true
    }

    pub fn cancel(&self) {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
        state.revision = state.revision.wrapping_add(1);
        if let Some(inflight) = &state.inflight {
            inflight.cancel();
        }
        state.pending = None;
        state.result = None;
        wake.notify_one();
    }

    pub fn take_result(&self) -> Option<Result<Vec<PredictionCandidate>, PredictionError>> {
        let mut state = self
            .mailbox
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .result
            .take()
            .filter(|(revision, _)| *revision == state.revision)
            .map(|(_, result)| result)
    }
}

impl Drop for PredictionWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
        state.stopped = true;
        if let Some(inflight) = &state.inflight {
            inflight.cancel();
        }
        state.pending = None;
        state.result = None;
        wake.notify_one();
        // Do not join a bounded in-flight network call on the UI/IBus thread.
    }
}

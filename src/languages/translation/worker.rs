//! One worker and one replaceable request. No text history, retries or UI-thread I/O.
use super::{TranslationError, TranslationProvider, TranslationRequest};
use crate::languages::llm::LlmProviderError;
use std::sync::{Arc, Condvar, Mutex};

struct Job {
    revision: u64,
    provider: Arc<dyn TranslationProvider>,
    request: TranslationRequest,
}

#[derive(Default)]
struct Mailbox {
    revision: u64,
    stopped: bool,
    pending: Option<Job>,
    result: Option<Result<String, TranslationError>>,
}

pub struct TranslationWorker {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
}

impl Default for TranslationWorker {
    fn default() -> Self {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_mailbox = mailbox.clone();
        let spawned = std::thread::Builder::new()
            .name("suzaku-translation".into())
            .spawn(move || {
                let (lock, wake) = &*worker_mailbox;
                loop {
                    let job = {
                        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                        while !state.stopped && state.pending.is_none() {
                            state = wake.wait(state).unwrap_or_else(|e| e.into_inner());
                        }
                        if state.stopped {
                            return;
                        }
                        state.pending.take().unwrap()
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        job.request.validate()?;
                        job.provider
                            .translate(&job.request)
                            .and_then(super::validate_translation_output)
                    }))
                    .unwrap_or(Err(TranslationError::Provider(
                        LlmProviderError::Unavailable,
                    )));
                    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                    if !state.stopped && state.revision == job.revision {
                        state.result = Some(result);
                    }
                }
            });
        if spawned.is_err() {
            mailbox.0.lock().unwrap().stopped = true;
        }
        Self { mailbox }
    }
}

impl TranslationWorker {
    pub fn request(
        &self,
        provider: Arc<dyn TranslationProvider>,
        request: TranslationRequest,
    ) -> bool {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopped {
            return false;
        }
        state.revision = state.revision.wrapping_add(1);
        state.result = None;
        state.pending = Some(Job {
            revision: state.revision,
            provider,
            request,
        });
        wake.notify_one();
        true
    }
    pub fn cancel(&self) {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.revision = state.revision.wrapping_add(1);
        state.pending = None;
        state.result = None;
        wake.notify_one();
    }
    pub fn take_result(&self) -> Option<Result<String, TranslationError>> {
        self.mailbox
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .result
            .take()
    }
}

impl Drop for TranslationWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.mailbox;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.stopped = true;
        state.pending = None;
        state.result = None;
        wake.notify_one();
        // Transport bounds in-flight calls. Never join a model request on the UI thread.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    struct Gated {
        entered: mpsc::Sender<String>,
        gate: Mutex<mpsc::Receiver<()>>,
    }
    impl TranslationProvider for Gated {
        fn translate(&self, request: &TranslationRequest) -> Result<String, TranslationError> {
            self.entered.send(request.text.clone()).unwrap();
            self.gate
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(3))
                .unwrap();
            Ok(format!("translated {}", request.text))
        }
    }
    #[test]
    fn explicit_latest_request_wins_and_cancel_never_replays_or_blocks() {
        let worker = TranslationWorker::default();
        let (entered, received) = mpsc::channel();
        let (gate, allowed) = mpsc::channel();
        let provider = Arc::new(Gated {
            entered,
            gate: Mutex::new(allowed),
        });
        let request = |text: &str| TranslationRequest {
            text: text.into(),
            source: None,
            target: Default::default(),
        };
        assert!(received.try_recv().is_err());
        assert!(worker.request(provider.clone(), request("old")));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            "old"
        );
        let start = Instant::now();
        worker.cancel();
        assert!(start.elapsed() < Duration::from_millis(100));
        worker.request(provider.clone(), request("superseded"));
        worker.request(provider.clone(), request("latest"));
        gate.send(()).unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            "latest"
        );
        assert!(worker.take_result().is_none());
        gate.send(()).unwrap();
        let start = Instant::now();
        let result = loop {
            if let Some(result) = worker.take_result() {
                break result;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        };
        assert_eq!(result, Ok("translated latest".into()));
        assert!(worker.take_result().is_none());
        assert!(received.try_recv().is_err());
        worker.request(provider, request("closing"));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(3)).unwrap(),
            "closing"
        );
        let start = Instant::now();
        drop(worker);
        assert!(start.elapsed() < Duration::from_millis(100));
        gate.send(()).unwrap();
    }
}

//! Single-flight, on-demand background probing for UI callers. Never probe while
//! holding the cache lock, never poll while idle, and never spawn per keystroke.
use super::{LinuxImeBootstrap, TargetPlatform};
use std::{
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

const CACHE_LIFETIME: Duration = Duration::from_secs(3);

#[derive(Default)]
struct CacheState {
    value: Option<LinuxImeBootstrap>,
    checked_at: Option<Instant>,
    in_flight: bool,
}

#[derive(Default)]
struct BackgroundCache(Mutex<CacheState>);

impl BackgroundCache {
    fn read(
        self: &Arc<Self>,
        platform: TargetPlatform,
        now: Instant,
        probe: impl FnOnce(TargetPlatform) -> LinuxImeBootstrap + Send + 'static,
    ) -> LinuxImeBootstrap {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut value = state
            .value
            .clone()
            .unwrap_or_else(|| pending_status(platform));
        // Linux profiles share the same local daemon; only their display tag differs.
        value.host_platform = platform;
        let refresh = !state.in_flight
            && state
                .checked_at
                .is_none_or(|checked| now.saturating_duration_since(checked) >= CACHE_LIFETIME);
        if refresh {
            state.in_flight = true;
            state.checked_at = Some(now);
        }
        drop(state);
        if refresh {
            let cache = Arc::clone(self);
            if std::thread::Builder::new()
                .name("suzaku-ime-status".into())
                .spawn(move || {
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe(platform)))
                            .unwrap_or_else(|_| pending_status(platform));
                    let mut state = cache
                        .0
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    state.value = Some(result);
                    state.checked_at = Some(Instant::now());
                    state.in_flight = false;
                })
                .is_err()
            {
                // Back off even if the OS refused a worker; do not retry on every frame.
                self.0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .in_flight = false;
            }
        }
        value
    }
}

fn pending_status(platform: TargetPlatform) -> LinuxImeBootstrap {
    LinuxImeBootstrap {
        framework: super::detected_framework(),
        host_platform: platform,
        daemon_detected: false,
        host_registration_ready: false,
        runtime_engine_visible: false,
        engine_active: false,
        host_service_ready: false,
        marked_text_roundtrip_ready: false,
        commit_roundtrip_ready: false,
        native_candidate_window_ready: false,
        recommended_connection_name: super::recommended_connection_name(),
    }
}

pub(super) fn status(platform: TargetPlatform) -> LinuxImeBootstrap {
    static CACHE: OnceLock<Arc<BackgroundCache>> = OnceLock::new();
    CACHE.get_or_init(Default::default).read(
        platform,
        Instant::now(),
        super::bootstrap_status_uncached,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };

    fn ready(platform: TargetPlatform) -> LinuxImeBootstrap {
        let mut value = pending_status(platform);
        value.host_service_ready = true;
        value
    }

    fn wait_finished(cache: &BackgroundCache) {
        let until = Instant::now() + Duration::from_secs(2);
        while cache.0.lock().unwrap().in_flight {
            assert!(Instant::now() < until, "background probe did not finish");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn cold_and_inflight_reads_return_without_waiting_for_the_probe() {
        let cache = Arc::new(BackgroundCache::default());
        let (release, blocked) = mpsc::channel();
        let started = Instant::now();
        let value = cache.read(TargetPlatform::Ubuntu, started, move |platform| {
            blocked.recv_timeout(Duration::from_secs(1)).unwrap();
            ready(platform)
        });
        assert!(!value.host_service_ready);
        for _ in 0..100 {
            assert!(
                !cache
                    .read(TargetPlatform::Ubuntu, started, |_| panic!(
                        "duplicate probe"
                    ))
                    .host_service_ready
            );
        }
        assert!(started.elapsed() < Duration::from_millis(200));
        release.send(()).unwrap();
        wait_finished(&cache);
        assert!(
            cache
                .read(TargetPlatform::Ubuntu, Instant::now(), |_| panic!(
                    "cached probe"
                ))
                .host_service_ready
        );
    }

    #[test]
    fn expired_cache_serves_last_result_and_coalesces_concurrent_refreshes() {
        let cache = Arc::new(BackgroundCache::default());
        let now = Instant::now();
        {
            let mut state = cache.0.lock().unwrap();
            state.value = Some(ready(TargetPlatform::Ubuntu));
            state.checked_at = Some(now - CACHE_LIFETIME);
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let (release, blocked) = mpsc::channel();
        let worker_calls = calls.clone();
        assert!(
            cache
                .read(TargetPlatform::Ubuntu, now, move |platform| {
                    worker_calls.fetch_add(1, Ordering::SeqCst);
                    blocked.recv_timeout(Duration::from_secs(1)).unwrap();
                    pending_status(platform)
                })
                .host_service_ready
        );
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    let value = cache.read(TargetPlatform::ArchLinux, now, |_| {
                        panic!("duplicate refresh")
                    });
                    assert_eq!(value.host_platform, TargetPlatform::ArchLinux);
                    assert!(value.host_service_ready);
                });
            }
        });
        release.send(()).unwrap();
        wait_finished(&cache);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            !cache
                .read(TargetPlatform::Ubuntu, Instant::now(), |_| panic!(
                    "retry loop"
                ))
                .host_service_ready
        );
    }

    #[test]
    fn failed_probe_releases_single_flight_and_backs_off() {
        let cache = Arc::new(BackgroundCache::default());
        cache.read(TargetPlatform::Ubuntu, Instant::now(), |_| {
            panic!("synthetic probe failure")
        });
        wait_finished(&cache);
        assert!(
            !cache
                .read(TargetPlatform::Ubuntu, Instant::now(), |_| panic!(
                    "immediate retry"
                ))
                .host_service_ready
        );
        cache.0.lock().unwrap().checked_at = Some(Instant::now() - CACHE_LIFETIME);
        cache.read(TargetPlatform::Ubuntu, Instant::now(), ready);
        wait_finished(&cache);
        assert!(
            cache
                .0
                .lock()
                .unwrap()
                .value
                .as_ref()
                .unwrap()
                .host_service_ready
        );
    }
}

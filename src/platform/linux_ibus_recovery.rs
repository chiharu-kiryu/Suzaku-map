//! Same-bus recovery after an observed host replacement. No keys, drafts or history.
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const RECOVERY_WINDOW: Duration = Duration::from_secs(8);

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Ready,
    EngineChanged,
    Recovered,
    Failed,
}

pub struct Monitor {
    stop: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Monitor {
    /// Only start after adopting the registered desktop host service. This observer
    /// never starts/stops services or IBus, and forgets history on bus replacement.
    pub fn start(socket: PathBuf, notify: impl Fn(Event) + Send + 'static) -> Option<Self> {
        let (stop, stopped) = mpsc::channel();
        let worker = std::thread::Builder::new().name("suzaku-engine-recovery".into()).spawn(move || {
            let mut reported = false;
            loop {
                match observe(&socket, &stopped, &notify) {
                    Ok(()) => break,
                    Err(()) if !reported => {
                        eprintln!("Suzaku engine recovery observer disconnected/unavailable; observation history was discarded");
                        reported = true;
                    },
                    Err(()) => {},
                }
                if stopped.recv_timeout(Duration::from_secs(2)) != Err(RecvTimeoutError::Timeout) { break; }
            }
        }).ok()?;
        Some(Self {
            stop: Some(stop),
            worker: Some(worker),
        })
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct History {
    owner: String,
    engine: Option<String>,
    pending: Option<(String, Instant)>,
}

fn valid_engine(name: &str) -> bool {
    !name.is_empty()
        && name != "dummy"
        && name.len() <= 256
        && !name.starts_with('-')
        && !name.chars().any(|c| c.is_whitespace() || c.is_control())
}

impl History {
    fn engine_changed(&mut self, name: String) {
        self.engine = valid_engine(&name).then_some(name);
        self.pending = None; // A newer selection (including a desktop fallback) wins.
    }

    fn owner_changed(&mut self, old: &str, new: &str, now: Instant) {
        if self.owner != old {
            return;
        }
        self.owner = new.to_owned();
        if !old.is_empty() {
            self.pending = self
                .engine
                .clone()
                .map(|name| (name, now + RECOVERY_WINDOW));
        }
    }

    fn candidate(&mut self, now: Instant) -> Option<String> {
        if self.pending.as_ref().is_some_and(|(_, end)| now >= *end) {
            self.pending = None;
        }
        if self.owner.is_empty() {
            return None;
        }
        self.pending.as_ref().map(|(name, _)| name.clone())
    }
}

enum Signal {
    Engine(String),
    Owner(String, String),
}
#[derive(Default)]
struct Signals {
    queue: VecDeque<Signal>,
    overflow: bool,
}

unsafe extern "C" {
    fn suzaku_ibus_recovery_new(
        address: *const c_char,
        callback: unsafe extern "C" fn(u32, *const c_char, *const c_char, *mut c_void),
        data: *mut c_void,
    ) -> *mut c_void;
    fn suzaku_ibus_recovery_free(bus: *mut c_void);
    fn suzaku_ibus_recovery_poll(bus: *mut c_void) -> bool;
    fn suzaku_ibus_recovery_query(
        bus: *mut c_void,
        operation: u32,
        out: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn suzaku_ibus_recovery_select(bus: *mut c_void, name: *const c_char);
}

unsafe extern "C" fn signal(
    kind: u32,
    first: *const c_char,
    second: *const c_char,
    data: *mut c_void,
) {
    // SAFETY: the native bridge invokes callbacks only during this thread's poll;
    // data is the stable boxed queue, and both strings are live NUL-terminated values.
    let (queue, first, second) = unsafe {
        (
            &mut *data.cast::<Signals>(),
            CStr::from_ptr(first),
            CStr::from_ptr(second),
        )
    };
    if queue.queue.len() >= 64 || first.to_bytes().len() > 256 || second.to_bytes().len() > 256 {
        queue.overflow = true;
        return;
    }
    let (Ok(first), Ok(second)) = (first.to_str(), second.to_str()) else {
        queue.overflow = true;
        return;
    };
    match kind {
        1 => queue.queue.push_back(Signal::Engine(first.into())),
        2 => queue
            .queue
            .push_back(Signal::Owner(first.into(), second.into())),
        _ => queue.overflow = true,
    }
}

struct Bus {
    raw: *mut c_void,
    signals: Box<Signals>,
}
impl Bus {
    fn connect() -> Result<Self, ()> {
        let output = super::linux_command::run(
            Command::new("ibus").arg("address"),
            Duration::from_millis(400),
        )
        .map_err(|_| ())?;
        if !output.status.success() {
            return Err(());
        }
        let address = std::str::from_utf8(&output.stdout).map_err(|_| ())?.trim();
        if !address.starts_with("unix:") || address.len() > 4096 || address.contains(';') {
            return Err(());
        }
        let address = CString::new(address).map_err(|_| ())?;
        let mut signals = Box::<Signals>::default();
        // SAFETY: native connection stays on this thread; the boxed callback data
        // remains valid until after native unsubscription/connection teardown.
        let raw = unsafe {
            suzaku_ibus_recovery_new(
                address.as_ptr(),
                signal,
                (&mut *signals as *mut Signals).cast(),
            )
        };
        if raw.is_null() {
            return Err(());
        }
        Ok(Self { raw, signals })
    }

    fn query(&self, operation: u32) -> Result<Option<String>, ()> {
        let mut text = [0 as c_char; 257];
        // SAFETY: the owned connection and writable buffer are live, with exact capacity.
        match unsafe {
            suzaku_ibus_recovery_query(self.raw, operation, text.as_mut_ptr(), text.len())
        } {
            0 => Ok(None),
            // SAFETY: the buffer starts zeroed and native copying is NUL-terminated.
            1 => unsafe { CStr::from_ptr(text.as_ptr()) }
                .to_str()
                .map(|s| Some(s.to_owned()))
                .map_err(|_| ()),
            _ => Err(()),
        }
    }

    fn engine(&self) -> Result<Option<String>, ()> {
        let name = self.query(1)?;
        if name.as_deref().is_some_and(|s| !valid_engine(s)) {
            return Err(());
        }
        Ok(name)
    }

    fn drain(&mut self, history: &mut History, notify: &impl Fn(Event)) -> Result<(), ()> {
        // SAFETY: polling and callback data access are confined to this worker thread.
        if !unsafe { suzaku_ibus_recovery_poll(self.raw) } || self.signals.overflow {
            return Err(());
        }
        for signal in self.signals.queue.drain(..) {
            match signal {
                Signal::Engine(name) => {
                    history.engine_changed(name);
                    notify(Event::EngineChanged);
                }
                Signal::Owner(old, new) => history.owner_changed(&old, &new, Instant::now()),
            }
        }
        Ok(())
    }

    fn select(&self, name: &str) -> Result<(), ()> {
        let name = CString::new(name).map_err(|_| ())?;
        // SAFETY: connection and NUL-terminated name live through this bounded call.
        unsafe {
            suzaku_ibus_recovery_select(self.raw, name.as_ptr());
        }
        Ok(())
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        // SAFETY: close/unsubscribe on the creating thread before dropping callback data.
        unsafe {
            suzaku_ibus_recovery_free(self.raw);
        }
    }
}

fn host_ready(socket: &Path) -> bool {
    let deadline = super::linux_ipc::Deadline::new(Duration::from_millis(120));
    let ready = || -> std::io::Result<bool> {
        let mut stream = deadline.connect(socket)?;
        deadline.send(&mut stream, b"Q")?;
        let mut byte = [0];
        deadline.read_exact(&mut stream, &mut byte)?;
        Ok(byte == *b"1")
    };
    ready().unwrap_or(false)
}

fn observe(socket: &Path, stopped: &Receiver<()>, notify: &impl Fn(Event)) -> Result<(), ()> {
    let mut bus = Bus::connect()?;
    let mut history = History {
        owner: bus.query(0)?.unwrap_or_default(),
        engine: bus.engine()?,
        pending: None,
    };
    notify(Event::Ready);
    loop {
        if stopped.recv_timeout(Duration::from_millis(100)) != Err(RecvTimeoutError::Timeout) {
            return Ok(());
        }
        bus.drain(&mut history, notify)?;
        let Some(target) = history.candidate(Instant::now()) else {
            continue;
        };
        if !host_ready(socket) || bus.query(0)?.as_deref() != Some(history.owner.as_str()) {
            continue;
        }
        if let Some(current) = bus.engine()? {
            history.engine_changed(current);
            notify(Event::EngineChanged);
            continue;
        }
        // Drain newer choices and recheck immediately before the sole bounded write.
        // Generic query errors abort observation; they are never a missing-engine signal.
        bus.drain(&mut history, notify)?;
        if history.candidate(Instant::now()).as_ref() != Some(&target) {
            continue;
        }
        if let Some(current) = bus.engine()? {
            history.engine_changed(current);
            notify(Event::EngineChanged);
            continue;
        }
        history.pending = None;
        bus.select(&target)?;
        match bus.engine()? {
            Some(current) => {
                let restored = current == target;
                history.engine_changed(current);
                notify(if restored {
                    Event::Recovered
                } else {
                    Event::EngineChanged
                });
            }
            None => notify(Event::Failed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_queue_is_bounded_and_invalid_names_invalidate_observation() {
        let mut queue = Box::<Signals>::default();
        let first = CString::new("rime").unwrap();
        let empty = CString::default();
        for _ in 0..65 {
            // SAFETY: live queue and NUL-terminated fixture names, called on this thread.
            unsafe {
                signal(
                    1,
                    first.as_ptr(),
                    empty.as_ptr(),
                    (&mut *queue as *mut Signals).cast(),
                );
            }
        }
        assert_eq!(queue.queue.len(), 64);
        assert!(queue.overflow);
        for name in [vec![b'x'; 257], vec![255]] {
            let mut queue = Box::<Signals>::default();
            let invalid = CString::new(name).unwrap();
            // SAFETY: as above; deliberately invalid UTF-8 is still a valid C string.
            unsafe {
                signal(
                    1,
                    invalid.as_ptr(),
                    empty.as_ptr(),
                    (&mut *queue as *mut Signals).cast(),
                );
            }
            assert!(queue.overflow);
            assert!(queue.queue.is_empty());
        }
    }

    #[test]
    fn a_stalled_private_bus_connection_is_canceled() {
        use std::{os::unix::net::UnixListener, time::SystemTime};
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "suzaku-recovery-{}-{unique}.sock",
            std::process::id()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let address = CString::new(format!("unix:path={}", path.display())).unwrap();
        let mut signals = Box::<Signals>::default();
        let start = Instant::now();
        // SAFETY: isolated listener never replies; the stable callback queue and
        // address outlive native connection setup and cancellation.
        let raw = unsafe {
            suzaku_ibus_recovery_new(
                address.as_ptr(),
                signal,
                (&mut *signals as *mut Signals).cast(),
            )
        };
        if !raw.is_null() {
            // SAFETY: release an unexpectedly successful fixture connection on its thread.
            unsafe {
                suzaku_ibus_recovery_free(raw);
            }
        }
        drop(listener);
        std::fs::remove_file(path).unwrap();
        assert!(raw.is_null());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn only_an_observed_replacement_can_restore_the_latest_engine() {
        let now = Instant::now();
        let mut state = History {
            owner: ":1.1".into(),
            ..Default::default()
        };
        state.engine_changed("rime".into());
        state.engine_changed("mozc-jp".into());
        assert_eq!(state.candidate(now), None);
        state.owner_changed(":1.1", "", now);
        assert_eq!(state.candidate(now), None);
        state.owner_changed("", ":1.2", now);
        assert_eq!(state.candidate(now).as_deref(), Some("mozc-jp"));
        state.engine_changed("xkb:us::eng".into());
        assert_eq!(state.candidate(now), None);
    }
    #[test]
    fn unknown_initial_state_wrong_owner_and_expired_recovery_never_switch() {
        let now = Instant::now();
        let mut state = History::default();
        state.owner_changed("", ":1.1", now);
        assert_eq!(state.candidate(now), None);
        state.engine_changed("rime".into());
        state.owner_changed(":1.wrong", "", now);
        assert_eq!(state.candidate(now), None);
        state.owner_changed(":1.1", ":1.2", now);
        assert_eq!(state.candidate(now + RECOVERY_WINDOW), None);
    }
    #[test]
    fn a_manual_selection_during_outage_cancels_recovery() {
        let now = Instant::now();
        for name in ["mozc-jp", "xkb:us::eng", "dummy", "", "bad\nname"] {
            let mut state = History {
                owner: ":1.1".into(),
                ..Default::default()
            };
            state.engine_changed("rime".into());
            state.owner_changed(":1.1", "", now);
            state.engine_changed(name.into());
            state.owner_changed("", ":1.2", now);
            assert_eq!(state.candidate(now), None);
        }
    }
}

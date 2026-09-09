use super::*;
use std::{
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    sync::{Arc, atomic::AtomicUsize},
    thread,
};

/// One private listener and one peer, never the desktop's native host.
pub(crate) struct Endpoint {
    root: PathBuf,
    pub path: PathBuf,
    pub listener: UnixListener,
}

impl Endpoint {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "suzaku-ipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("host.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        Self {
            root,
            path,
            listener,
        }
    }
    pub fn fill_backlog(&self) -> UnixStream {
        // SAFETY: valid owned listener fd; a backlog of zero admits one peer.
        assert_eq!(unsafe { libc::listen(self.listener.as_raw_fd(), 0) }, 0);
        Deadline::new(Duration::from_millis(200))
            .connect(&self.path)
            .unwrap()
    }
    pub fn accept(&self) -> UnixStream {
        let deadline = Deadline::new(Duration::from_secs(1));
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => return stream,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => deadline
                    .wait(self.listener.as_raw_fd(), libc::POLLIN)
                    .unwrap(),
                Err(e) => panic!("private fixture accept failed: {e}"),
            }
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        // Only the two exact paths created by this fixture, never recursive.
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.root);
    }
}

#[test]
fn saturated_connect_obeys_deadline_and_leaves_no_late_request() {
    let endpoint = Endpoint::new();
    let _queued = endpoint.fill_backlog();
    let started = Instant::now();
    for _ in 0..8 {
        assert_eq!(
            Deadline::new(Duration::from_millis(10))
                .connect(&endpoint.path)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }
    assert!(started.elapsed() < Duration::from_millis(500));
    drop(endpoint.accept());
    assert_eq!(
        endpoint.listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    // The same listener works as soon as its queue is drained.
    let mut client = Deadline::new(Duration::from_millis(200))
        .connect(&endpoint.path)
        .unwrap();
    let mut server = endpoint.accept();
    let deadline = Deadline::new(Duration::from_millis(200));
    deadline.send(&mut client, b"Q").unwrap();
    assert_eq!(deadline.read_to_end(&mut server, 1).unwrap(), b"Q");
    deadline.send(&mut server, b"1").unwrap();
    let mut ack = [0];
    deadline.read_exact(&mut client, &mut ack).unwrap();
    assert_eq!(ack, *b"1");
}

#[test]
fn cancellation_interrupts_a_saturated_connect_and_an_idle_read() {
    let endpoint = Endpoint::new();
    let _queued = endpoint.fill_backlog();
    for connecting in [true, false] {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            signal.store(true, Ordering::Release);
        });
        let started = Instant::now();
        let deadline = Deadline::cancellable(Duration::from_secs(2), &stop);
        let error = if connecting {
            deadline.connect(&endpoint.path).unwrap_err()
        } else {
            let (mut reader, _writer) = UnixStream::pair().unwrap();
            deadline.read(&mut reader, &mut [0]).unwrap_err()
        };
        worker.join().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
        assert!(started.elapsed() < Duration::from_millis(300));
    }
}

#[test]
fn trickled_reads_cannot_reset_the_total_deadline() {
    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        writer
            .set_write_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        for _ in 0..30 {
            if writer.write_all(b" ").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    let started = Instant::now();
    let error = Deadline::new(Duration::from_millis(45))
        .read_to_end(&mut reader, 8192)
        .unwrap_err();
    drop(reader);
    worker.join().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_millis(250));
}

#[test]
fn stalled_writes_are_bounded_and_expired_requests_send_nothing() {
    let (mut writer, mut reader) = UnixStream::pair().unwrap();
    let expired = Deadline::new(Duration::ZERO);
    assert_eq!(
        expired
            .send(&mut writer, b"Cshould not be sent")
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    reader.set_nonblocking(true).unwrap();
    assert_eq!(
        reader.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let started = Instant::now();
    let error = Deadline::new(Duration::from_millis(45))
        .send(&mut writer, &vec![b'x'; 1_048_576])
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_millis(300));
}

#[test]
fn responses_preserve_exact_bytes_and_enforce_size_and_eof() {
    for limit in [8, 9] {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let deadline = Deadline::new(Duration::from_millis(100));
        deadline.send(&mut writer, " \t日本\n".as_bytes()).unwrap();
        let response = deadline.read_to_end(&mut reader, limit);
        if limit == 8 {
            assert_eq!(response.unwrap_err().kind(), io::ErrorKind::InvalidData);
        } else {
            assert_eq!(response.unwrap(), " \t日本\n".as_bytes());
        }
    }
    let (mut reader, writer) = UnixStream::pair().unwrap();
    drop(writer);
    assert_eq!(
        Deadline::new(Duration::from_millis(100))
            .read_exact(&mut reader, &mut [0])
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn invalid_or_missing_socket_paths_fail_without_retrying() {
    for path in [
        Path::new(""),
        Path::new("bad\0path"),
        Path::new(&"x".repeat(108)),
    ] {
        assert_eq!(
            Deadline::new(Duration::from_secs(1))
                .connect(path)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    let endpoint = Endpoint::new();
    assert_eq!(
        Deadline::new(Duration::from_secs(1))
            .connect(&endpoint.root.join("missing.sock"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}

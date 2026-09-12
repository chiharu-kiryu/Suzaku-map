//! Finite loopback fixtures; no desktop services or external endpoints.
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

pub(super) fn accept(listener: &TcpListener) -> (TcpStream, SocketAddr) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                return (stream, address);
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(2))
            }
            Err(error) => panic!("fixture accept timed out or failed: {error}"),
        }
    }
}

pub(super) fn read_request(stream: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > 64 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if let Some(split) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..split]);
            let length = header
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if bytes.len() >= split + 4 + length {
                return String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into());
            }
        }
    }
}

pub(super) fn respond(stream: &mut impl Write, status: &str, body: &str) -> io::Result<()> {
    // A client may close immediately after reading a non-200 status. Formatting
    // directly into the socket leaves subsequent header/body writes racing it.
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn small_error_responses_are_written_before_the_peer_can_close_on_the_status_line() {
    struct StatusClosingPeer {
        wire: Vec<u8>,
        closed: bool,
    }
    impl Write for StatusClosingPeer {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.closed {
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            self.wire.extend_from_slice(buffer);
            self.closed = self.wire.windows(2).any(|part| part == b"\r\n");
            Ok(buffer.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut peer = StatusClosingPeer {
        wire: Vec::new(),
        closed: false,
    };
    respond(&mut peer, "404 Not Found", "synthetic error").unwrap();
    assert!(peer.closed);
    let wire = String::from_utf8(peer.wire).unwrap();
    assert!(wire.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(wire.ends_with("\r\n\r\nsynthetic error"));
}

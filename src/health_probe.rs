use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Probe the `/health` endpoint over a minimal raw TCP stream.
/// Returns true if HTTP 200 OK was received within 2 seconds.
pub fn run_health_probe(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(2)) else {
        eprintln!("Health check failed: connection to {addr} failed");
        return false;
    };

    if stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .is_err()
        || stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .is_err()
    {
        eprintln!("Health check failed: failed to set socket timeouts");
        return false;
    }

    let req =
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        eprintln!("Health check failed: failed to write request");
        return false;
    }

    let mut buf = [0u8; 128];
    let Ok(n) = stream.read(&mut buf) else {
        eprintln!("Health check failed: failed to read response");
        return false;
    };

    let response = String::from_utf8_lossy(&buf[..n]);
    if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        true
    } else {
        let first_line = response.lines().next().unwrap_or("");
        eprintln!("Health check failed: status response was '{first_line}'");
        false
    }
}

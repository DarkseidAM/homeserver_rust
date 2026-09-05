use homeserver::health_probe::run_health_probe;
use std::io::Write;
use std::net::TcpListener;

#[test]
fn test_health_probe_fails_on_unreachable_port() {
    let success = run_health_probe(59999);
    assert!(!success);
}

#[test]
fn test_health_probe_succeeds_on_200_ok() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 256];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let success = run_health_probe(port);
    handle.join().unwrap();
    assert!(success);
}

#[test]
fn test_health_probe_fails_on_500_error() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 256];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\nERROR";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let success = run_health_probe(port);
    handle.join().unwrap();
    assert!(!success);
}

//! A minimal raw HTTP/1.1 client for `list`/`--dump` to talk to an already-running instance's
//! Read API on localhost. The API's payloads are small and simple by design (acceptance #3) and
//! this only ever talks to our own server, so a hand-rolled `TcpStream` client keeps this at zero
//! new dependencies instead of pulling in a full HTTP client crate for a handful of local GETs.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

pub struct ApiError(pub String);

/// GETs `path` from the Read API at `addr` (`host:port`) and returns the parsed JSON body.
pub fn get_json(addr: &str, path: &str) -> Result<serde_json::Value, ApiError> {
    let mut stream = TcpStream::connect(addr).map_err(|e| {
        ApiError(format!(
            "no {} instance found at {addr} ({e}) -- start one with `{}`",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_NAME")
        ))
    })?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();

    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|e| ApiError(format!("write to {addr}: {e}")))?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| ApiError(format!("read from {addr}: {e}")))?;

    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| ApiError(format!("malformed HTTP response from {addr}{path}")))?;

    let headers = String::from_utf8_lossy(&raw[..split]);
    let status_line = headers.lines().next().unwrap_or("");
    if !status_line.contains(" 200 ") {
        return Err(ApiError(format!("{path} returned `{status_line}`")));
    }

    let body = &raw[split + 4..];
    serde_json::from_slice(body).map_err(|e| ApiError(format!("parsing response from {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Spawns a one-shot listener that hands back a fixed HTTP response to the first
    /// connection, then returns its address.
    fn one_shot_server(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr").to_string();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        addr
    }

    #[test]
    fn parses_the_json_body_of_a_200_response() {
        let addr = one_shot_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"ok\":true}",
        );

        let body = get_json(&addr, "/api/traces").unwrap_or_else(|e| panic!("{}", e.0));
        assert_eq!(body, serde_json::json!({"ok": true}));
    }

    #[test]
    fn a_non_200_status_becomes_a_readable_error() {
        let addr = one_shot_server("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");

        let err = get_json(&addr, "/api/traces/deadbeef").unwrap_err();
        assert!(err.0.contains("404"), "expected the status line in the error: {}", err.0);
    }

    #[test]
    fn connection_refused_names_the_fix() {
        // bind then immediately drop, so the port is very likely free but nothing is listening.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr").to_string();
        drop(listener);

        let err = get_json(&addr, "/api/traces").unwrap_err();
        assert!(err.0.contains("no spanfall instance found"), "unexpected message: {}", err.0);
    }
}

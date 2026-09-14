//! T16 (spec 07): when a listen port is already taken, name the process holding it instead of a
//! bare "address already in use" -- the first-run failure anyone with Jaeger or the OTel
//! Collector already running is guaranteed to hit.

use std::process::Command;

/// Best-effort: shells out to `lsof` to find the pid+command bound to `port` in LISTEN state.
/// `None` if `lsof` is missing, errors, or finds nothing -- callers fall back to the raw OS error.
pub fn describe_holder(port: u16) -> Option<String> {
    let output = Command::new("lsof").args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    // line 0 is the header (COMMAND PID USER FD TYPE ...); the first data line is enough.
    let line = text.lines().nth(1)?;
    let mut fields = line.split_whitespace();
    let command = fields.next()?;
    let pid = fields.next()?;
    Some(format!("{command} (pid {pid})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lsof_available() -> bool {
        Command::new("lsof").arg("-v").output().is_ok()
    }

    #[test]
    fn names_the_process_holding_a_listening_port() {
        if !lsof_available() {
            eprintln!("skipping: lsof not available in this environment");
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("local_addr").port();

        // lsof looking at a socket this same process just bound is not a permissions edge case
        // -- if this comes back None, describe_holder is actually broken, not merely sandboxed.
        let holder = describe_holder(port).unwrap_or_else(|| panic!("lsof found no holder for our own listener on port {port}"));
        assert!(holder.contains("pid"), "expected a pid in {holder}");
        drop(listener);
    }

    #[test]
    fn an_unbound_port_has_no_holder() {
        if !lsof_available() {
            eprintln!("skipping: lsof not available in this environment");
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("local_addr").port();
        drop(listener);

        assert!(describe_holder(port).is_none());
    }
}

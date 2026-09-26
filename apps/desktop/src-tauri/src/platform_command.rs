//! Bounded local IPC commands. Arguments never pass through a shell; diagnostics
//! never contain command output (which can include private window titles).
use std::{
    io::{ErrorKind, Read},
    os::fd::AsRawFd,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) const OUTPUT_LIMIT: usize = 1024 * 1024;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) struct ManagedChild(pub Child);

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(crate) fn spawn(program: &str, args: &[&str]) -> Result<ManagedChild, String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map(ManagedChild)
        .map_err(|_| "IPC command unavailable; check installation and session permissions".into())
}

pub(crate) fn run(program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    run_with_limits(program, args, TIMEOUT, OUTPUT_LIMIT)
}

fn run_with_limits(
    program: &str,
    args: &[&str],
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut child = spawn(program, args)?;
    let mut stdout = child.0.stdout.take().ok_or("IPC output unavailable")?;
    let fd = stdout.as_raw_fd();
    // SAFETY: fd belongs to the live stdout pipe; F_GETFL/F_SETFL do not retain it.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("IPC pipe configuration failed".into());
    }
    let started = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    let mut eof = false;
    loop {
        if started.elapsed() >= timeout {
            return Err("IPC command timed out".into());
        }
        match stdout.read(&mut buffer) {
            Ok(0) => eof = true,
            Ok(count) => {
                if output.len() + count > limit {
                    return Err("IPC output exceeded the size limit".into());
                }
                output.extend_from_slice(&buffer[..count]);
                continue;
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err("IPC output read failed".into()),
        }
        if let Some(status) = child.0.try_wait().map_err(|_| "IPC status unavailable")? {
            if !status.success() {
                return Err("IPC command failed; check the running desktop integration".into());
            }
            if eof {
                return Ok(output);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_timeout_output_and_redacts_failure() {
        assert_eq!(run("sh", &["-c", "printf ok"]).unwrap(), b"ok");
        let error = run("sh", &["-c", "printf secret; printf private >&2; exit 1"]).unwrap_err();
        assert!(!error.contains("secret") && !error.contains("private"));
        assert!(run_with_limits("sh", &["-c", "printf toolong"], TIMEOUT, 3)
            .unwrap_err()
            .contains("size limit"));
        let started = Instant::now();
        assert!(run_with_limits(
            "sh",
            &["-c", "exec sleep 10"],
            Duration::from_millis(50),
            32
        )
        .unwrap_err()
        .contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

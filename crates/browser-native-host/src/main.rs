//! Minimal Phase 2 native-messaging framing probe.
//!
//! The production browser policy remains a Phase 3 integration. This helper
//! only validates the length-prefixed JSON transport and never executes a
//! browser-provided command.
use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut len = [0; 4];
        if input.read_exact(&mut len).is_err() {
            break;
        }
        let size = u32::from_le_bytes(len) as usize;
        if size > 1024 * 1024 {
            break;
        }
        let mut body = vec![0; size];
        input.read_exact(&mut body)?;
        // Transport-only acknowledgement. Policy and pairing belong to the
        // desktop backend and are not implemented in this Phase 2 helper.
        let response = br#"{"version":1,"type":"heartbeat_ack"}"#;
        output.write_all(&(response.len() as u32).to_le_bytes())?;
        output.write_all(response)?;
        output.flush()?;
    }
    Ok(())
}

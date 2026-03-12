use anyhow::Result;
use std::io::Write;

use crate::session::resolve_session;
use crate::session::ring_buffer::RingBuffer;

/// Run `latch history <name|id>`
/// Reads the ring buffer for the session and dumps it to stdout.
pub fn run(target: &str) -> Result<()> {
    let (id, _meta) = resolve_session(target)?;
    let paths = crate::session::SessionPaths::new(&id);

    if !paths.history.exists() {
        anyhow::bail!("No history found for session '{}'", target);
    }

    let buf = RingBuffer::open(&paths.history)?;
    let data = buf.read_all();

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&data)?;
    stdout.flush()?;

    Ok(())
}

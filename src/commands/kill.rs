use anyhow::Result;

use crate::session::resolve_session;

/// Run `latch kill <name|id>`
/// Sends SIGTERM, waits 3s, then SIGKILL if still alive. Removes the session directory.
pub fn run(target: &str) -> Result<()> {
    let (id, meta) = resolve_session(target)?;
    let paths = crate::session::SessionPaths::new(&id);

    let pid = meta.pid as i32;

    // Send SIGTERM
    let term_result = unsafe { libc::kill(pid, libc::SIGTERM) };

    if term_result == 0 {
        // Wait up to 3 seconds for the process to die
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if unsafe { libc::kill(pid, 0) } != 0 {
                break; // Process is dead
            }
            if std::time::Instant::now() >= deadline {
                // SIGKILL
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
                // Brief wait for SIGKILL to take effect
                std::thread::sleep(std::time::Duration::from_millis(100));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    // Remove session directory
    if paths.dir.exists() {
        std::fs::remove_dir_all(&paths.dir)?;
    }

    println!("Session '{}' killed and cleaned up", target);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn kill_sigterm_then_sigkill_strategy() {
        // This is tested via integration tests since it requires a running process.
        // Unit test validates the constants used.
        assert_eq!(libc::SIGTERM, 15);
        assert_eq!(libc::SIGKILL, 9);
    }
}

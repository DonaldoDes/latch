use anyhow::Result;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::session::{sessions_dir, SessionMeta, SessionStatus};

/// Information about a session for display
#[derive(Debug)]
pub struct SessionInfo {
    pub name: String,
    pub cmd: String,
    pub pid: u32,
    pub status: LiveStatus,
    pub is_current: bool,
}

/// Live status determined by probing the socket and PID
#[derive(Debug, PartialEq)]
pub enum LiveStatus {
    Attached,
    Detached,
    Dead,
}

/// Check if a PID is alive
fn pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Determine live status for a session
pub fn check_liveness(meta: &SessionMeta, socket_path: &Path) -> LiveStatus {
    // First check if PID is alive
    if !pid_alive(meta.pid) {
        return LiveStatus::Dead;
    }

    // Try to connect to socket
    match UnixStream::connect(socket_path) {
        Ok(_) => {
            // Socket is reachable — check meta status
            match meta.status {
                SessionStatus::Attached => LiveStatus::Attached,
                _ => LiveStatus::Detached,
            }
        }
        Err(_) => LiveStatus::Dead,
    }
}

/// Format a session line for display
pub fn format_session_line(info: &SessionInfo) -> String {
    let marker = if info.is_current { "*" } else { " " };
    let status_icon = match info.status {
        LiveStatus::Attached => "\u{25CF}", // filled circle
        LiveStatus::Detached => "\u{25CB}", // empty circle
        LiveStatus::Dead => "\u{2717}",     // cross mark
    };

    if info.status == LiveStatus::Dead {
        format!(
            "{} {:<6} [{}] {:<6} (dead)",
            marker, info.name, status_icon, info.cmd
        )
    } else {
        format!(
            "{} {:<6} [{}] {:<6} pid={}",
            marker, info.name, status_icon, info.cmd, info.pid
        )
    }
}

/// Collect all sessions and their live status
pub fn collect_sessions() -> Result<Vec<SessionInfo>> {
    let base = sessions_dir();
    if !base.exists() {
        return Ok(Vec::new());
    }

    let current_session = std::env::var("LATCH_SESSION").ok();
    let mut sessions = Vec::new();

    for entry in std::fs::read_dir(&base)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        let socket_path = entry.path().join("server.sock");

        if !meta_path.exists() {
            continue;
        }

        if let Ok(meta) = SessionMeta::read_from(&meta_path) {
            let status = check_liveness(&meta, &socket_path);
            let is_current = current_session.as_deref() == Some(&meta.name);

            sessions.push(SessionInfo {
                name: meta.name,
                cmd: meta.cmd,
                pid: meta.pid,
                status,
                is_current,
            });
        }
    }

    // Sort: current first, then alphabetically by name
    sessions.sort_by(|a, b| b.is_current.cmp(&a.is_current).then(a.name.cmp(&b.name)));

    Ok(sessions)
}

/// Run `latch list`
pub fn run() -> Result<()> {
    let sessions = collect_sessions()?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    for info in &sessions {
        println!("{}", format_session_line(info));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_attached_session() {
        let info = SessionInfo {
            name: "work".to_string(),
            cmd: "bash".to_string(),
            pid: 1234,
            status: LiveStatus::Attached,
            is_current: true,
        };
        let line = format_session_line(&info);
        assert!(line.starts_with("*"));
        assert!(line.contains("work"));
        assert!(line.contains("\u{25CF}")); // filled circle
        assert!(line.contains("bash"));
        assert!(line.contains("pid=1234"));
    }

    #[test]
    fn format_detached_session() {
        let info = SessionInfo {
            name: "dev".to_string(),
            cmd: "zsh".to_string(),
            pid: 5678,
            status: LiveStatus::Detached,
            is_current: false,
        };
        let line = format_session_line(&info);
        assert!(line.starts_with(" "));
        assert!(line.contains("dev"));
        assert!(line.contains("\u{25CB}")); // empty circle
        assert!(line.contains("pid=5678"));
    }

    #[test]
    fn format_dead_session() {
        let info = SessionInfo {
            name: "old".to_string(),
            cmd: "bash".to_string(),
            pid: 9999,
            status: LiveStatus::Dead,
            is_current: false,
        };
        let line = format_session_line(&info);
        assert!(line.contains("\u{2717}")); // cross mark
        assert!(line.contains("(dead)"));
        assert!(!line.contains("pid="));
    }

    #[test]
    fn current_session_has_star_prefix() {
        let info = SessionInfo {
            name: "cur".to_string(),
            cmd: "bash".to_string(),
            pid: 1,
            status: LiveStatus::Attached,
            is_current: true,
        };
        let line = format_session_line(&info);
        assert!(line.starts_with("*"));
    }

    #[test]
    fn non_current_session_has_space_prefix() {
        let info = SessionInfo {
            name: "other".to_string(),
            cmd: "bash".to_string(),
            pid: 1,
            status: LiveStatus::Detached,
            is_current: false,
        };
        let line = format_session_line(&info);
        assert!(line.starts_with(" "));
    }
}

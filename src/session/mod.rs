pub mod ring_buffer;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Session status as stored in meta.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Detached,
    Attached,
    Dead,
}

/// Session metadata persisted as meta.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub cmd: String,
    pub pid: u32,
    pub created_at: String,
    pub status: SessionStatus,
}

/// Paths for a session's runtime files
#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub dir: PathBuf,
    pub socket: PathBuf,
    pub history: PathBuf,
    pub meta: PathBuf,
}

/// Returns the base directory for all sessions using platform-appropriate paths.
/// Uses `directories::ProjectDirs` to resolve the data directory:
/// - macOS: ~/Library/Application Support/latch/sessions/
/// - Linux: ~/.local/share/latch/sessions/
///
/// Returns an error path if HOME is unset (ProjectDirs returns None).
pub fn sessions_base_dir() -> PathBuf {
    match directories::ProjectDirs::from("", "", "latch") {
        Some(proj_dirs) => proj_dirs.data_dir().join("sessions"),
        None => {
            eprintln!("[latch] error: cannot determine data directory (HOME unset?)");
            // Return a path that will fail on use rather than silently using /tmp
            PathBuf::from("/nonexistent-latch-data-dir/sessions")
        }
    }
}

/// Returns the base directory for sessions, overridable for tests via LATCH_DATA_DIR
pub fn sessions_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LATCH_DATA_DIR") {
        PathBuf::from(dir).join("sessions")
    } else {
        sessions_base_dir()
    }
}

impl SessionPaths {
    pub fn new(id: &str) -> Self {
        let dir = sessions_dir().join(id);
        Self {
            socket: dir.join("server.sock"),
            history: dir.join("history.bin"),
            meta: dir.join("meta.json"),
            dir,
        }
    }

    pub fn from_base(base: &Path, id: &str) -> Self {
        let dir = base.join(id);
        Self {
            socket: dir.join("server.sock"),
            history: dir.join("history.bin"),
            meta: dir.join("meta.json"),
            dir,
        }
    }

    /// Create the session directory if it doesn't exist, with mode 0700
    pub fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create session dir: {:?}", self.dir))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("Failed to set permissions on session dir: {:?}", self.dir))
    }
}

impl SessionMeta {
    /// Write meta.json to the given path
    pub fn write_to(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Read meta.json from the given path
    pub fn read_from(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read meta.json: {:?}", path))?;
        let meta: Self = serde_json::from_str(&data)?;
        Ok(meta)
    }

    /// Update status in the meta.json file
    pub fn update_status(path: &Path, status: SessionStatus) -> Result<()> {
        let mut meta = Self::read_from(path)?;
        meta.status = status;
        meta.write_to(path)
    }
}

/// Generate a random session id: "session-<6 hex chars>"
pub fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let hex: String = (0..6)
        .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
        .collect();
    format!("session-{}", hex)
}

/// Resolve a session name or id prefix to a full session id
pub fn resolve_session(target: &str) -> Result<(String, SessionMeta)> {
    resolve_session_in(&sessions_dir(), target)
}

/// Resolve a session within a specific base directory
pub fn resolve_session_in(base: &Path, target: &str) -> Result<(String, SessionMeta)> {
    if !base.exists() {
        anyhow::bail!("No sessions found");
    }

    let mut matches = Vec::new();
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        if meta_path.exists() {
            if let Ok(meta) = SessionMeta::read_from(&meta_path) {
                // Exact name match
                if meta.name == target {
                    return Ok((meta.id.clone(), meta));
                }
                // Id prefix match (at least 4 chars)
                if target.len() >= 4 && meta.id.starts_with(target) {
                    matches.push(meta);
                }
            }
        }
    }

    match matches.len() {
        0 => anyhow::bail!("No session found matching '{}'", target),
        1 => {
            let meta = matches
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("unexpected empty matches"))?;
            Ok((meta.id.clone(), meta))
        }
        _ => anyhow::bail!(
            "Ambiguous session '{}': matches {} sessions",
            target,
            matches.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn session_paths_have_correct_structure() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        let paths = SessionPaths::from_base(base, "abc123");

        assert_eq!(paths.dir, base.join("abc123"));
        assert_eq!(paths.socket, base.join("abc123/server.sock"));
        assert_eq!(paths.history, base.join("abc123/history.bin"));
        assert_eq!(paths.meta, base.join("abc123/meta.json"));
    }

    #[test]
    fn ensure_dir_creates_session_directory() {
        let tmp = TempDir::new().unwrap();
        let paths = SessionPaths::from_base(tmp.path(), "test-session");
        assert!(!paths.dir.exists());
        paths.ensure_dir().unwrap();
        assert!(paths.dir.exists());
    }

    #[test]
    fn meta_json_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join("meta.json");
        let meta = SessionMeta {
            id: "session-abc123".to_string(),
            name: "mysession".to_string(),
            cmd: "bash".to_string(),
            pid: 12345,
            created_at: "2026-03-12T10:00:00Z".to_string(),
            status: SessionStatus::Detached,
        };

        meta.write_to(&meta_path).unwrap();
        let loaded = SessionMeta::read_from(&meta_path).unwrap();

        assert_eq!(loaded.id, "session-abc123");
        assert_eq!(loaded.name, "mysession");
        assert_eq!(loaded.cmd, "bash");
        assert_eq!(loaded.pid, 12345);
        assert_eq!(loaded.status, SessionStatus::Detached);
    }

    #[test]
    fn meta_json_contains_all_required_fields() {
        let meta = SessionMeta {
            id: "session-abc123".to_string(),
            name: "test".to_string(),
            cmd: "bash".to_string(),
            pid: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            status: SessionStatus::Detached,
        };

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"cmd\""));
        assert!(json.contains("\"pid\""));
        assert!(json.contains("\"created_at\""));
        assert!(json.contains("\"status\""));
    }

    #[test]
    fn status_serializes_lowercase() {
        let meta = SessionMeta {
            id: "x".to_string(),
            name: "x".to_string(),
            cmd: "x".to_string(),
            pid: 1,
            created_at: "x".to_string(),
            status: SessionStatus::Detached,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"detached\""));
    }

    #[test]
    fn update_status_changes_meta_file() {
        let tmp = TempDir::new().unwrap();
        let meta_path = tmp.path().join("meta.json");
        let meta = SessionMeta {
            id: "s1".to_string(),
            name: "test".to_string(),
            cmd: "bash".to_string(),
            pid: 100,
            created_at: "now".to_string(),
            status: SessionStatus::Detached,
        };
        meta.write_to(&meta_path).unwrap();

        SessionMeta::update_status(&meta_path, SessionStatus::Attached).unwrap();
        let updated = SessionMeta::read_from(&meta_path).unwrap();
        assert_eq!(updated.status, SessionStatus::Attached);
    }

    #[test]
    fn generate_session_id_has_correct_format() {
        let id = generate_session_id();
        assert!(id.starts_with("session-"));
        assert_eq!(id.len(), "session-".len() + 6);
        // All chars after prefix are hex
        for c in id["session-".len()..].chars() {
            assert!(c.is_ascii_hexdigit());
        }
    }

    #[test]
    fn resolve_session_by_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        std::fs::create_dir_all(&base).unwrap();
        let session_dir = base.join("session-aaa111");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta = SessionMeta {
            id: "session-aaa111".to_string(),
            name: "work".to_string(),
            cmd: "bash".to_string(),
            pid: 1,
            created_at: "now".to_string(),
            status: SessionStatus::Detached,
        };
        meta.write_to(&session_dir.join("meta.json")).unwrap();

        let (id, resolved) = resolve_session_in(&base, "work").unwrap();
        assert_eq!(id, "session-aaa111");
        assert_eq!(resolved.name, "work");
    }

    #[test]
    fn resolve_session_by_id_prefix() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        std::fs::create_dir_all(&base).unwrap();
        let session_dir = base.join("session-bbb222");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta = SessionMeta {
            id: "session-bbb222".to_string(),
            name: "dev".to_string(),
            cmd: "zsh".to_string(),
            pid: 2,
            created_at: "now".to_string(),
            status: SessionStatus::Detached,
        };
        meta.write_to(&session_dir.join("meta.json")).unwrap();

        let (id, _) = resolve_session_in(&base, "sess").unwrap();
        assert_eq!(id, "session-bbb222");
    }
}

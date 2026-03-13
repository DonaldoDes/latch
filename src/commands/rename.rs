use anyhow::Result;

use crate::session::{resolve_session, sessions_dir, SessionMeta, SessionPaths};

/// Check if a session name already exists
fn name_exists(name: &str) -> Result<bool> {
    name_exists_in(&sessions_dir(), name)
}

/// Check if a session name already exists in a given base dir
fn name_exists_in(base: &std::path::Path, name: &str) -> Result<bool> {
    if !base.exists() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        if meta_path.exists() {
            if let Ok(meta) = SessionMeta::read_from(&meta_path) {
                if meta.name == name {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Run `latch rename <name|id> <new-name>`
pub fn run(target: &str, new_name: &str) -> Result<()> {
    // Check for name conflict
    if name_exists(new_name)? {
        anyhow::bail!("error: session '{}' already exists", new_name);
    }

    let (id, _meta) = resolve_session(target)?;
    run_by_id(&id, new_name)
}

/// Rename a session by its ID (used by TUI)
pub fn run_by_id(session_id: &str, new_name: &str) -> Result<()> {
    // Check for name conflict
    if name_exists(new_name)? {
        anyhow::bail!("error: session '{}' already exists", new_name);
    }

    let paths = SessionPaths::new(session_id);
    if !paths.meta.exists() {
        anyhow::bail!("Session '{}' not found", session_id);
    }

    let mut meta = SessionMeta::read_from(&paths.meta)?;
    let old_name = meta.name.clone();
    meta.name = new_name.to_string();
    meta.write_to(&paths.meta)?;

    println!("Session '{}' renamed to '{}'", old_name, new_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionMeta, SessionStatus};
    use tempfile::TempDir;

    fn create_session(base: &std::path::Path, id: &str, name: &str) {
        let dir = base.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let meta = SessionMeta {
            id: id.to_string(),
            name: name.to_string(),
            cmd: "bash".to_string(),
            pid: 1000,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            status: SessionStatus::Detached,
        };
        meta.write_to(&dir.join("meta.json")).unwrap();
    }

    #[test]
    fn name_exists_returns_true_for_existing() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        create_session(&base, "session-aaa111", "work");

        assert!(name_exists_in(&base, "work").unwrap());
    }

    #[test]
    fn name_exists_returns_false_for_missing() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        create_session(&base, "session-aaa111", "work");

        assert!(!name_exists_in(&base, "nothere").unwrap());
    }

    #[test]
    fn name_exists_returns_false_for_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("nonexistent");
        assert!(!name_exists_in(&base, "work").unwrap());
    }

    #[test]
    #[serial_test::serial]
    fn rename_updates_meta_json() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        create_session(&base, "session-aaa111", "old");
        std::env::set_var("LATCH_DATA_DIR", tmp.path().to_str().unwrap());

        run_by_id("session-aaa111", "new").unwrap();

        let meta = SessionMeta::read_from(&base.join("session-aaa111/meta.json")).unwrap();
        assert_eq!(meta.name, "new");

        std::env::remove_var("LATCH_DATA_DIR");
    }

    #[test]
    #[serial_test::serial]
    fn rename_fails_on_name_conflict() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        create_session(&base, "session-aaa111", "work");
        create_session(&base, "session-bbb222", "dev");
        std::env::set_var("LATCH_DATA_DIR", tmp.path().to_str().unwrap());

        let result = run_by_id("session-bbb222", "work");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(err, "error: session 'work' already exists");

        // Original unchanged
        let meta = SessionMeta::read_from(&base.join("session-bbb222/meta.json")).unwrap();
        assert_eq!(meta.name, "dev");

        std::env::remove_var("LATCH_DATA_DIR");
    }

    #[test]
    #[serial_test::serial]
    fn rename_via_run_resolves_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("sessions");
        create_session(&base, "session-aaa111", "mywork");
        std::env::set_var("LATCH_DATA_DIR", tmp.path().to_str().unwrap());

        run("mywork", "myproject").unwrap();

        let meta = SessionMeta::read_from(&base.join("session-aaa111/meta.json")).unwrap();
        assert_eq!(meta.name, "myproject");

        std::env::remove_var("LATCH_DATA_DIR");
    }
}

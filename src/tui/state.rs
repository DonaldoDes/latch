use crate::session::SessionMeta;

/// TUI interaction modes
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    NewSession {
        input: String,
    },
    Filter {
        input: String,
    },
    Help,
    KillConfirm {
        session_id: String,
        name: String,
    },
    Rename {
        session_id: String,
        current_name: String,
        input: String,
    },
}

/// Session status as displayed in the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Current,
    Attached,
    Detached,
    Dead,
}

/// A session entry displayed in the TUI
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub meta: SessionMeta,
    pub status: SessionStatus,
}

/// TUI application state
pub struct AppState {
    pub sessions: Vec<SessionEntry>,
    pub selected: usize,
    pub mode: Mode,
    pub filtered_indices: Option<Vec<usize>>,
}

/// Action to be performed after handling an event
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Quit,
    Attach {
        session_id: String,
    },
    NewSession {
        name: String,
    },
    KillSession {
        session_id: String,
    },
    CleanupDead {
        session_id: String,
    },
    RenameSession {
        session_id: String,
        new_name: String,
    },
}

impl AppState {
    pub fn new(sessions: Vec<SessionEntry>) -> Self {
        Self {
            sessions,
            selected: 0,
            mode: Mode::Normal,
            filtered_indices: None,
        }
    }

    /// Get the visible sessions (filtered or all)
    pub fn visible_sessions(&self) -> Vec<&SessionEntry> {
        match &self.filtered_indices {
            Some(indices) => indices
                .iter()
                .filter_map(|&i| self.sessions.get(i))
                .collect(),
            None => self.sessions.iter().collect(),
        }
    }

    /// Get the currently selected session entry
    pub fn selected_entry(&self) -> Option<&SessionEntry> {
        let visible = self.visible_sessions();
        visible.get(self.selected).copied()
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let len = self.visible_sessions().len();
        if len > 0 {
            self.selected = (self.selected + 1) % len;
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        let len = self.visible_sessions().len();
        if len > 0 {
            self.selected = if self.selected == 0 {
                len - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Apply a filter string to sessions
    pub fn apply_filter(&mut self, filter: &str) {
        if filter.is_empty() {
            self.filtered_indices = None;
        } else {
            let lower = filter.to_lowercase();
            let indices: Vec<usize> = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| s.meta.name.to_lowercase().contains(&lower))
                .map(|(i, _)| i)
                .collect();
            self.filtered_indices = Some(indices);
        }
        // Clamp selected
        let len = self.visible_sessions().len();
        if len > 0 && self.selected >= len {
            self.selected = len - 1;
        }
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filtered_indices = None;
        let len = self.visible_sessions().len();
        if len > 0 && self.selected >= len {
            self.selected = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionMeta;

    fn make_entry(name: &str, status: SessionStatus) -> SessionEntry {
        SessionEntry {
            meta: SessionMeta {
                id: format!("session-{}", name),
                name: name.to_string(),
                cmd: "bash".to_string(),
                pid: 1000,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                status: crate::session::SessionStatus::Detached,
            },
            status,
        }
    }

    fn make_app(names: &[(&str, SessionStatus)]) -> AppState {
        let entries: Vec<SessionEntry> = names
            .iter()
            .map(|(n, s)| make_entry(n, s.clone()))
            .collect();
        AppState::new(entries)
    }

    // --- Mode tests ---

    #[test]
    fn initial_mode_is_normal() {
        let app = make_app(&[("work", SessionStatus::Detached)]);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn initial_selected_is_zero() {
        let app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        assert_eq!(app.selected, 0);
    }

    // --- Navigation tests ---

    #[test]
    fn move_down_increments_selected() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
            ("c", SessionStatus::Detached),
        ]);
        app.move_down();
        assert_eq!(app.selected, 1);
        app.move_down();
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn move_down_wraps_around() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        app.move_down();
        app.move_down();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn move_up_decrements_selected() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
            ("c", SessionStatus::Detached),
        ]);
        app.selected = 2;
        app.move_up();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_up_wraps_around() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        app.move_up();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_on_empty_sessions_does_not_panic() {
        let mut app = AppState::new(vec![]);
        app.move_down();
        assert_eq!(app.selected, 0);
        app.move_up();
        assert_eq!(app.selected, 0);
    }

    // --- Filter tests ---

    #[test]
    fn filter_reduces_visible_sessions() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
            ("workspace", SessionStatus::Attached),
        ]);
        app.apply_filter("work");
        let visible = app.visible_sessions();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].meta.name, "work");
        assert_eq!(visible[1].meta.name, "workspace");
    }

    #[test]
    fn empty_filter_shows_all() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.apply_filter("");
        assert_eq!(app.visible_sessions().len(), 2);
    }

    #[test]
    fn filter_clamps_selected() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
            ("staging", SessionStatus::Detached),
        ]);
        app.selected = 2;
        app.apply_filter("dev");
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn clear_filter_restores_all() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.apply_filter("dev");
        assert_eq!(app.visible_sessions().len(), 1);
        app.clear_filter();
        assert_eq!(app.visible_sessions().len(), 2);
    }

    #[test]
    fn filter_is_case_insensitive() {
        let mut app = make_app(&[
            ("WorkSpace", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.apply_filter("workspace");
        assert_eq!(app.visible_sessions().len(), 1);
    }

    // --- Selected entry tests ---

    #[test]
    fn selected_entry_returns_correct_session() {
        let app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Attached),
        ]);
        let entry = app.selected_entry().unwrap();
        assert_eq!(entry.meta.name, "a");
    }

    #[test]
    fn selected_entry_returns_none_on_empty() {
        let app = AppState::new(vec![]);
        assert!(app.selected_entry().is_none());
    }

    // --- SessionStatus tests ---

    #[test]
    fn session_status_variants_distinct() {
        assert_ne!(SessionStatus::Current, SessionStatus::Attached);
        assert_ne!(SessionStatus::Attached, SessionStatus::Detached);
        assert_ne!(SessionStatus::Detached, SessionStatus::Dead);
        assert_ne!(SessionStatus::Dead, SessionStatus::Current);
    }

    // --- Mode variants tests ---

    #[test]
    fn mode_new_session_holds_input() {
        let mode = Mode::NewSession {
            input: "mywork".to_string(),
        };
        if let Mode::NewSession { input } = mode {
            assert_eq!(input, "mywork");
        } else {
            panic!("expected NewSession");
        }
    }

    #[test]
    fn mode_kill_confirm_holds_session_info() {
        let mode = Mode::KillConfirm {
            session_id: "session-abc".to_string(),
            name: "work".to_string(),
        };
        if let Mode::KillConfirm { session_id, name } = mode {
            assert_eq!(session_id, "session-abc");
            assert_eq!(name, "work");
        } else {
            panic!("expected KillConfirm");
        }
    }

    #[test]
    fn mode_rename_holds_current_and_input() {
        let mode = Mode::Rename {
            session_id: "session-abc".to_string(),
            current_name: "old".to_string(),
            input: "new".to_string(),
        };
        if let Mode::Rename {
            session_id,
            current_name,
            input,
        } = mode
        {
            assert_eq!(session_id, "session-abc");
            assert_eq!(current_name, "old");
            assert_eq!(input, "new");
        } else {
            panic!("expected Rename");
        }
    }

    #[test]
    fn mode_filter_holds_input() {
        let mode = Mode::Filter {
            input: "dev".to_string(),
        };
        if let Mode::Filter { input } = mode {
            assert_eq!(input, "dev");
        } else {
            panic!("expected Filter");
        }
    }
}

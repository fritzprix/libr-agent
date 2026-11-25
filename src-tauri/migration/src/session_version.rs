//! Shared definition of the `sessions.version` message-change triggers (#2878).
//!
//! The trigger set is the correctness basis of the headless write-back's
//! merge-or-fork fork base, so migration `m20261117_000053` and the
//! write-back's legacy-profile install path must use exactly these
//! statements — never a copy.

pub const SESSION_VERSION_TRIGGER_NAMES: [&str; 3] = [
    "messages_session_version_ai",
    "messages_session_version_au",
    "messages_session_version_ad",
];

pub const SESSION_VERSION_TRIGGERS: [&str; 3] = [
    "CREATE TRIGGER IF NOT EXISTS messages_session_version_ai AFTER INSERT ON messages BEGIN
      UPDATE sessions SET version = version + 1 WHERE id = NEW.session_id;
    END;",
    "CREATE TRIGGER IF NOT EXISTS messages_session_version_au AFTER UPDATE ON messages BEGIN
      UPDATE sessions SET version = version + 1 WHERE id = NEW.session_id;
    END;",
    "CREATE TRIGGER IF NOT EXISTS messages_session_version_ad AFTER DELETE ON messages BEGIN
      UPDATE sessions SET version = version + 1 WHERE id = OLD.session_id;
    END;",
];

//! `CLAUDE_ENV_FILE` support (#2926).
//!
//! A `SessionStart` hook may write `NAME=value` lines to the file named by
//! `CLAUDE_ENV_FILE`; every later shell tool call of that session must then
//! see those variables. The file itself is per session
//! ([`super::registry::get_session_plugin_env_file`]); the parsed values are
//! kept here in memory, keyed by session, and merged into the environment of
//! each host/docker command that session spawns.
//!
//! Spec: <https://code.claude.com/docs/en/hooks> (`CLAUDE_ENV_FILE`).
//! Only plain `NAME=value` lines are honoured. Anything else (a shell
//! statement, a function, a subshell, a line without `=`) is skipped: the file
//! is read on the host, so it is never executed.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

fn store() -> &'static RwLock<BTreeMap<String, BTreeMap<String, String>>> {
    static STORE: once_cell::sync::Lazy<RwLock<BTreeMap<String, BTreeMap<String, String>>>> =
        once_cell::sync::Lazy::new(|| RwLock::new(BTreeMap::new()));
    &STORE
}

/// Whether `name` is a portable environment-variable name (`POSIX`-style:
/// letter or `_`, then letters, digits, `_`).
fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Strip one matching pair of surrounding single or double quotes, the way a
/// shell would read `NAME="a b"`.
fn unquote(value: &str) -> &str {
    let trimmed = value.trim();
    for quote in ['"', '\''] {
        if trimmed.len() >= 2 && trimmed.starts_with(quote) && trimmed.ends_with(quote) {
            return &trimmed[1..trimmed.len() - 1];
        }
    }
    trimmed
}

/// Parse the plain `NAME=value` lines of an env file. Later lines win, blank
/// lines and `#` comments are skipped, and an invalid line is skipped rather
/// than failing the whole file.
pub fn parse_env_file(raw: &str) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if !is_env_name(name) {
            log::debug!("CLAUDE_ENV_FILE: skipping line with invalid name {name:?}");
            continue;
        }
        vars.insert(name.to_string(), unquote(value).to_string());
    }
    vars
}

/// Read `path` and make its variables the session's plugin environment.
/// A missing or unreadable file simply leaves the session with no plugin
/// environment (a hook that writes nothing is the normal case).
pub fn apply_env_file(session_id: &str, path: &Path) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            set_session_plugin_env(session_id, BTreeMap::new());
            return;
        }
        Err(error) => {
            log::warn!(
                "CLAUDE_ENV_FILE {} for session {} could not be read: {}",
                path.display(),
                session_id,
                error
            );
            return;
        }
    };
    let vars = parse_env_file(&raw);
    log::debug!(
        "CLAUDE_ENV_FILE: session {} gained {} plugin variable(s)",
        session_id,
        vars.len()
    );
    set_session_plugin_env(session_id, vars);
}

/// Replace the plugin environment of one session.
pub fn set_session_plugin_env(session_id: &str, vars: BTreeMap<String, String>) {
    if let Ok(mut store) = store().write() {
        if vars.is_empty() {
            store.remove(session_id);
        } else {
            store.insert(session_id.to_string(), vars);
        }
    }
}

/// The plugin environment of one session (empty when it has none).
pub fn session_plugin_env(session_id: &str) -> BTreeMap<String, String> {
    store()
        .read()
        .ok()
        .and_then(|store| store.get(session_id).cloned())
        .unwrap_or_default()
}

/// Drop a session's plugin environment (session end / termination).
pub fn clear_session_plugin_env(session_id: &str) {
    if let Ok(mut store) = store().write() {
        store.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_name_value_lines() {
        let raw = "# comment\n\nFOO=bar\nexport BAZ='a b'\nQUX=\"x y\"\nEMPTY=\n";
        let vars = parse_env_file(raw);
        assert_eq!(vars.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(vars.get("BAZ").map(String::as_str), Some("a b"));
        assert_eq!(vars.get("QUX").map(String::as_str), Some("x y"));
        assert_eq!(vars.get("EMPTY").map(String::as_str), Some(""));
    }

    #[test]
    fn skips_statements_and_invalid_names() {
        let raw = "rm -rf /\n1BAD=x\nGOOD=1\nnot a var\n";
        let vars = parse_env_file(raw);
        assert_eq!(vars.len(), 1, "only GOOD=1 is a variable: {vars:?}");
        assert_eq!(vars.get("GOOD").map(String::as_str), Some("1"));
    }

    #[test]
    fn later_lines_win() {
        let vars = parse_env_file("A=1\nA=2\n");
        assert_eq!(vars.get("A").map(String::as_str), Some("2"));
    }

    #[test]
    fn applying_a_missing_file_clears_the_session() {
        let session = "test-env-file-missing";
        set_session_plugin_env(session, BTreeMap::from([("A".into(), "1".into())]));
        apply_env_file(session, Path::new("/nonexistent/libragent/env-file"));
        assert!(session_plugin_env(session).is_empty());
    }

    #[test]
    fn applying_a_file_replaces_the_session_variables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session-start.env");
        std::fs::write(&path, "TOKEN=abc\nEXPORTED=yes\n").unwrap();
        let session = "test-env-file-apply";
        apply_env_file(session, &path);
        let vars = session_plugin_env(session);
        assert_eq!(vars.get("TOKEN").map(String::as_str), Some("abc"));
        assert_eq!(vars.get("EXPORTED").map(String::as_str), Some("yes"));

        std::fs::write(&path, "TOKEN=changed\n").unwrap();
        apply_env_file(session, &path);
        let vars = session_plugin_env(session);
        assert_eq!(vars.get("TOKEN").map(String::as_str), Some("changed"));
        assert!(
            !vars.contains_key("EXPORTED"),
            "a re-run replaces, not merges, the previous variables"
        );
        clear_session_plugin_env(session);
        assert!(session_plugin_env(session).is_empty());
    }
}

//! Session IDs: one string for storage, prompts, and tools.
//!
//! # Contract
//!
//! - **New sessions**: [`generate_session_id`] → bare 10-hex. Display == storage.
//! - **Lookup (read only)**: exact match first; if missing, accept legacy refs
//!   that agents/URLs/HTTP clients used before unification:
//!   bare last-10 suffix, or optional `session-{suffix}` / `session-{full}`.
//!   Ambiguous suffixes fail closed.

use std::fmt;

/// Length of canonical new session ids and of the legacy suffix matcher.
pub const SESSION_ID_SHORT_LEN: usize = 10;

/// Historical prefix still accepted on **input** only (`session-{id-or-suffix}`).
pub const SESSION_ID_LEGACY_PREFIX: &str = "session-";

/// Opaque storage session key (`sessions.id` / `SessionMetadata.id`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageSessionId(String);

impl StorageSessionId {
    pub fn from_resolved(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StorageSessionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for StorageSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Outcome of resolving a session reference against candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdResolve<'a> {
    Unique(&'a str),
    Missing,
    Ambiguous(usize),
}

/// New session id: 10 lowercase hex chars.
#[must_use]
pub fn generate_session_id() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    hex[..SESSION_ID_SHORT_LEN].to_string()
}

/// Agent/HTTP-facing id — identical to the storage key (no truncation).
#[must_use]
pub fn display_session_id(session_id: &str) -> String {
    session_id.to_string()
}

/// Last [`SESSION_ID_SHORT_LEN`] chars of the unique part (after optional `session-`).
fn legacy_short_suffix(session_id: &str) -> String {
    let unique = session_id
        .strip_prefix(SESSION_ID_LEGACY_PREFIX)
        .unwrap_or(session_id);
    let char_count = unique.chars().count();
    if char_count <= SESSION_ID_SHORT_LEN {
        return unique.to_string();
    }
    unique
        .chars()
        .skip(char_count - SESSION_ID_SHORT_LEN)
        .collect()
}

/// Whether `input_ref` refers to `stored_id` (exact or legacy short/prefix form).
pub fn session_id_matches_legacy_ref(stored_id: &str, input_ref: &str) -> bool {
    if stored_id == input_ref {
        return true;
    }
    let input_token = input_ref
        .strip_prefix(SESSION_ID_LEGACY_PREFIX)
        .unwrap_or(input_ref);
    let stored_suffix = legacy_short_suffix(stored_id);
    input_token == stored_suffix || stored_id == input_token
}

/// Whether a miss on exact lookup might still resolve via legacy short / `session-` forms.
///
/// Skip full-table scans when the ref cannot be a legacy alias (e.g. a long bare id that
/// simply does not exist).
#[must_use]
pub fn should_try_legacy_session_resolve(input_ref: &str) -> bool {
    let trimmed = input_ref.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with(SESSION_ID_LEGACY_PREFIX) {
        return true;
    }
    trimmed.chars().count() == SESSION_ID_SHORT_LEN
}

/// Resolve a session reference among known storage ids.
///
/// Exact match wins. Otherwise unique legacy short/prefix match among candidates.
pub fn resolve_session_id_among<'a>(
    candidate_ids: impl IntoIterator<Item = &'a str>,
    input_ref: &str,
) -> SessionIdResolve<'a> {
    let candidates: Vec<&str> = candidate_ids.into_iter().collect();

    if let Some(exact) = candidates.iter().copied().find(|id| *id == input_ref) {
        return SessionIdResolve::Unique(exact);
    }

    let matches: Vec<&str> = candidates
        .iter()
        .copied()
        .filter(|id| session_id_matches_legacy_ref(id, input_ref))
        .collect();

    match matches.as_slice() {
        [only] => SessionIdResolve::Unique(only),
        [] => SessionIdResolve::Missing,
        many => SessionIdResolve::Ambiguous(many.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_identity() {
        assert_eq!(
            display_session_id("sum4n7z4fksfku0he02eoe9m"),
            "sum4n7z4fksfku0he02eoe9m"
        );
        assert_eq!(display_session_id("a1b2c3d4e5"), "a1b2c3d4e5");
    }

    #[test]
    fn resolve_exact_and_legacy_short_suffix() {
        let ids = ["sum4n7z4fksfku0he02eoe9m", "a1b2c3d4e5"];
        assert_eq!(
            resolve_session_id_among(ids, "sum4n7z4fksfku0he02eoe9m"),
            SessionIdResolve::Unique("sum4n7z4fksfku0he02eoe9m")
        );
        assert_eq!(
            resolve_session_id_among(ids, "0he02eoe9m"),
            SessionIdResolve::Unique("sum4n7z4fksfku0he02eoe9m")
        );
        assert_eq!(
            resolve_session_id_among(ids, "a1b2c3d4e5"),
            SessionIdResolve::Unique("a1b2c3d4e5")
        );
    }

    #[test]
    fn resolve_legacy_session_prefix() {
        let ids = ["session-1735123456789012345"];
        assert_eq!(
            resolve_session_id_among(ids, "6789012345"),
            SessionIdResolve::Unique("session-1735123456789012345")
        );
        assert_eq!(
            resolve_session_id_among(ids, "session-6789012345"),
            SessionIdResolve::Unique("session-1735123456789012345")
        );
    }

    #[test]
    fn resolve_ambiguous_short_suffix() {
        let ids = ["xxxx6789012345", "yyyy6789012345"];
        assert_eq!(
            resolve_session_id_among(ids, "6789012345"),
            SessionIdResolve::Ambiguous(2)
        );
    }

    #[test]
    fn legacy_resolve_gate() {
        assert!(should_try_legacy_session_resolve("0he02eoe9m"));
        assert!(should_try_legacy_session_resolve("session-6789012345"));
        assert!(should_try_legacy_session_resolve("a1b2c3d4e5"));
        assert!(!should_try_legacy_session_resolve(
            "sum4n7z4fksfku0he02eoe9m"
        ));
        assert!(!should_try_legacy_session_resolve(""));
        assert!(!should_try_legacy_session_resolve("   "));
    }
}

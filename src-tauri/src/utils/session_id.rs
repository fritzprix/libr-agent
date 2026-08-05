//! Session ID display aliases and reverse lookup.
//!
//! # Contract
//!
//! - **Storage keys** (opaque): legacy `session-<timestamp…>`, or modern bare 10-hex
//!   spawn ids (`a1b2c3d4e5`). Never rewrite existing rows.
//! - **Agent/HTTP-facing display**: always the short token only (no `session-` prefix),
//!   where the token is the last [`SESSION_ID_SHORT_LEN`] chars of the unique part
//!   (or the whole unique part when shorter). Legacy storage keys never render as a
//!   useless truncated `session-` label.
//! - **Tool/API input acceptance**: full stored id, optional `session-{short}` form,
//!   or bare short token.
//! - **Resolution order**: exact stored-id match first; otherwise display alias / short
//!   token among a caller-provided candidate set (MCP tools scope this to the caller's
//!   **delegated descendants**). Zero matches → missing; multiple → ambiguous.

/// Length of the unique short token used in display aliases.
pub const SESSION_ID_SHORT_LEN: usize = 10;

/// Historical / optional prefix still accepted on input (`session-{short}`).
/// Display output no longer includes this prefix.
pub const SESSION_ID_DISPLAY_PREFIX: &str = "session-";

/// Outcome of resolving an agent-supplied session reference against candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdResolve<'a> {
    /// Exactly one stored id matched (exact or alias).
    Unique(&'a str),
    /// No candidate matched.
    Missing,
    /// More than one candidate matched the same alias/token.
    Ambiguous(usize),
}

/// Extract the short unique token from a stored session id.
///
/// - Bare short ids (`a1b2c3d4e5`) → themselves
/// - Prefixed ids (`session-…`) → last [`SESSION_ID_SHORT_LEN`] chars of the unique part
pub fn session_id_short_token(session_id: &str) -> String {
    let unique = session_id
        .strip_prefix(SESSION_ID_DISPLAY_PREFIX)
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

/// External session reference: short token only (no `session-` prefix).
///
/// Idempotent for values that are already a short display token.
pub fn display_session_id(session_id: &str) -> String {
    session_id_short_token(session_id)
}

/// Whether `input_ref` (full id, optional `session-{short}`, or bare short token)
/// refers to `stored_id`.
pub fn session_id_matches_ref(stored_id: &str, input_ref: &str) -> bool {
    if stored_id == input_ref {
        return true;
    }
    if display_session_id(stored_id) == input_ref {
        return true;
    }
    let input_token = input_ref
        .strip_prefix(SESSION_ID_DISPLAY_PREFIX)
        .unwrap_or(input_ref);
    let stored_token = session_id_short_token(stored_id);
    input_token == stored_token || stored_id == input_token
}

/// Resolve an agent-supplied session reference against known candidate ids.
///
/// Prefer exact stored-id match. Otherwise match display alias / short token.
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
        .filter(|id| session_id_matches_ref(id, input_ref))
        .collect();

    match matches.as_slice() {
        [only] => SessionIdResolve::Unique(*only),
        [] => SessionIdResolve::Missing,
        many => SessionIdResolve::Ambiguous(many.len()),
    }
}

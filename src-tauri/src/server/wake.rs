//! Per-session wake tokens for process-completion callbacks.
//!
//! A long-running process started by a session calls
//! `POST /api/sessions/<id>/wake/<token>` when it finishes. The token is minted
//! lazily per session, kept only in memory, and revoked when the session is
//! terminated or deleted, so callbacks from orphaned processes (or after an app
//! restart) are rejected. Tokens must never be logged or shown to the model:
//! shells receive the full URL through [`WAKE_URL_ENV`] instead, and tool
//! results are passed through [`redact_wake_tokens`] before they reach the
//! model, the UI or stored history.
//!
//! The token is a routing credential for the callback, not the Session API's
//! security boundary. `POST /api/sessions/<id>/messages` intentionally stays
//! token-less: the UI and external clients use it, and the local access guard
//! (localhost + Host/Origin checks) protects the whole Session API. Revisit
//! this when the Session API gets its own authentication (#2235).

use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;

/// Environment variable carrying this session's wake URL into shell processes.
pub const WAKE_URL_ENV: &str = "LIBRAGENT_WAKE_URL";

const MAX_SUMMARY_CHARS: usize = 2_000;
const MAX_LOG_PATH_CHARS: usize = 1_024;

static WAKE_TOKENS: Lazy<DashMap<String, String>> = Lazy::new(DashMap::new);

const REDACTED_TOKEN: &str = "[redacted]";

/// Any wake URL path, including tokens of other or revoked sessions.
static WAKE_PATH_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/wake/[0-9a-f]{64}").expect("valid wake token regex"));

fn mint_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Returns the session's wake token, minting one on first use.
pub fn wake_token_for(session_id: &str) -> String {
    WAKE_TOKENS
        .entry(session_id.to_string())
        .or_insert_with(mint_token)
        .clone()
}

/// Full wake URL for a session (contains the secret token).
pub fn wake_url_for(session_id: &str) -> String {
    format!(
        "http://127.0.0.1:{}/api/sessions/{}/wake/{}",
        super::active_http_port(),
        session_id,
        wake_token_for(session_id)
    )
}

/// Wake URL a Docker container can reach: the wake-only listener on the host, through
/// `host.docker.internal`. `None` when that listener is not running.
pub fn container_wake_url_for(session_id: &str) -> Option<String> {
    let port = super::wake_listener::wake_listener_port()?;
    Some(container_wake_url(
        port,
        session_id,
        &wake_token_for(session_id),
    ))
}

fn container_wake_url(port: u16, session_id: &str, token: &str) -> String {
    format!(
        "http://{}:{port}/api/sessions/{session_id}/wake/{token}",
        super::wake_listener::CONTAINER_HOST_NAME
    )
}

/// Invalidates the session's token; later callbacks with it are rejected.
pub fn revoke_wake_token(session_id: &str) {
    WAKE_TOKENS.remove(session_id);
}

/// Constant-time check of `token` against the session's current token.
pub fn verify_wake_token(session_id: &str, token: &str) -> bool {
    let Some(expected) = WAKE_TOKENS.get(session_id) else {
        return false;
    };
    let expected = expected.as_bytes();
    let given = token.as_bytes();
    if expected.len() != given.len() {
        return false;
    }
    expected
        .iter()
        .zip(given)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Replaces wake tokens in `text`: every `/wake/<token>` path, plus the
/// session's current token wherever it appears on its own.
pub fn redact_wake_tokens<'a>(session_id: &str, text: &'a str) -> Cow<'a, str> {
    let redacted = WAKE_PATH_TOKEN.replace_all(text, "/wake/[redacted]");
    match WAKE_TOKENS.get(session_id) {
        Some(token) if redacted.contains(token.as_str()) => {
            Cow::Owned(redacted.replace(token.as_str(), REDACTED_TOKEN))
        }
        _ => redacted,
    }
}

/// [`redact_wake_tokens`] applied to every string inside a JSON value.
pub fn redact_wake_tokens_in_value(session_id: &str, value: &mut Value) {
    match value {
        Value::String(text) => {
            if let Cow::Owned(redacted) = redact_wake_tokens(session_id, text) {
                *text = redacted;
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact_wake_tokens_in_value(session_id, item)),
        Value::Object(map) => map
            .values_mut()
            .for_each(|item| redact_wake_tokens_in_value(session_id, item)),
        _ => {}
    }
}

/// Callback body. Every field is optional so `curl -X POST "$LIBRAGENT_WAKE_URL"`
/// with no body still wakes the session.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakePayload {
    pub status: Option<String>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub log_path: Option<String>,
}

fn truncate_chars(value: &str, max: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max).collect();
    out.push('…');
    out
}

impl WakePayload {
    /// `success` / `failure` from `status`, else derived from `exitCode`.
    fn resolved_status(&self) -> &str {
        match self.status.as_deref().map(str::trim) {
            Some(status) if !status.is_empty() => status,
            _ => match self.exit_code {
                Some(0) => "success",
                Some(_) => "failure",
                None => "unknown",
            },
        }
    }

    /// Message delivered to the woken session.
    pub fn to_message(&self) -> String {
        let mut lines = vec![format!(
            "[wake callback] Background process finished: status={}",
            truncate_chars(self.resolved_status(), 32)
        )];
        if let Some(code) = self.exit_code {
            lines.push(format!("exitCode: {code}"));
        }
        if let Some(path) = self.log_path.as_deref().filter(|p| !p.trim().is_empty()) {
            lines.push(format!(
                "logPath: {}",
                truncate_chars(path, MAX_LOG_PATH_CHARS)
            ));
        }
        if let Some(summary) = self.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            lines.push(format!(
                "summary: {}",
                truncate_chars(summary, MAX_SUMMARY_CHARS)
            ));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_url_targets_the_host_name_and_is_redacted() {
        let id = "wake-test-container-url";
        let token = wake_token_for(id);
        let url = container_wake_url(4567, id, &token);

        assert_eq!(
            url,
            format!("http://host.docker.internal:4567/api/sessions/{id}/wake/{token}")
        );
        let redacted = redact_wake_tokens(id, &url);
        assert!(!redacted.contains(&token));
        revoke_wake_token(id);
    }

    #[test]
    fn no_container_url_until_the_wake_listener_runs() {
        assert!(super::super::wake_listener::wake_listener_port().is_none());
        assert!(container_wake_url_for("wake-test-no-listener").is_none());
    }

    #[test]
    fn token_is_stable_until_revoked() {
        let id = "wake-test-stable";
        let first = wake_token_for(id);
        assert_eq!(first.len(), 64);
        assert_eq!(first, wake_token_for(id));
        assert!(verify_wake_token(id, &first));

        revoke_wake_token(id);
        assert!(!verify_wake_token(id, &first));
        assert_ne!(first, wake_token_for(id));
        revoke_wake_token(id);
    }

    #[test]
    fn token_is_bound_to_its_session() {
        let a = wake_token_for("wake-test-a");
        wake_token_for("wake-test-b");
        assert!(!verify_wake_token("wake-test-b", &a));
        assert!(!verify_wake_token("wake-test-unknown", &a));
        assert!(!verify_wake_token("wake-test-a", ""));
        assert!(!verify_wake_token("wake-test-a", &a[..63]));
        revoke_wake_token("wake-test-a");
        revoke_wake_token("wake-test-b");
    }

    #[test]
    fn redacts_wake_url_and_bare_token() {
        let id = "wake-test-redact";
        let token = wake_token_for(id);
        let other = wake_token_for("wake-test-redact-other");
        let text = format!(
            "LIBRAGENT_WAKE_URL=http://127.0.0.1:3030/api/sessions/{id}/wake/{token}\n\
             tail={token}\nother=/wake/{other}"
        );

        let redacted = redact_wake_tokens(id, &text);
        assert!(!redacted.contains(&token));
        assert!(!redacted.contains(&other));
        assert!(redacted.contains(&format!("/api/sessions/{id}/wake/[redacted]")));
        assert!(redacted.contains("tail=[redacted]"));

        assert!(matches!(
            redact_wake_tokens(id, "no secrets here"),
            Cow::Borrowed(_)
        ));

        let mut value = serde_json::json!({ "stdout": [text], "exitCode": 0 });
        redact_wake_tokens_in_value(id, &mut value);
        assert!(!value.to_string().contains(&token));
        assert_eq!(value["exitCode"], 0);

        revoke_wake_token(id);
        revoke_wake_token("wake-test-redact-other");
    }

    #[test]
    fn message_derives_status_from_exit_code() {
        let payload = WakePayload {
            exit_code: Some(2),
            log_path: Some("build.log".into()),
            ..WakePayload::default()
        };
        let message = payload.to_message();
        assert!(message.contains("status=failure"));
        assert!(message.contains("exitCode: 2"));
        assert!(message.contains("logPath: build.log"));

        let ok = WakePayload {
            exit_code: Some(0),
            ..WakePayload::default()
        };
        assert!(ok.to_message().contains("status=success"));
        assert!(WakePayload::default()
            .to_message()
            .contains("status=unknown"));
    }

    #[test]
    fn message_truncates_long_summary() {
        let payload = WakePayload {
            summary: Some("x".repeat(MAX_SUMMARY_CHARS + 50)),
            ..WakePayload::default()
        };
        let message = payload.to_message();
        assert!(message.ends_with('…'));
        assert!(message.len() < MAX_SUMMARY_CHARS + 200);
    }
}

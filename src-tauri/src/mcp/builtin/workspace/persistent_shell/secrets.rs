//! Session-scoped shell secrets (#2544).
//!
//! A value the user gives once through a `storeAs` prompt is kept in memory for the
//! session and passed to the persistent shell as an environment variable. Values never
//! go into logs, tool results, or chat history; shell output is masked before it is
//! returned.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::mcp::types::{MCPContent, MCPResult};

/// `name -> value` for one session.
pub type SecretMap = BTreeMap<String, String>;

/// Stored values of every live session, so the logger can mask lines that carry no
/// session id (webview logs, process debug lines).
#[derive(Default)]
struct LogMaskRegistry {
    sessions: HashMap<String, SecretMap>,
    /// Every form a value can take in a log line, longest first. Rebuilt on each change
    /// so a log line only scans it.
    patterns: Vec<String>,
}

impl LogMaskRegistry {
    fn rebuild_patterns(&mut self) {
        let mut patterns: Vec<String> = self
            .sessions
            .values()
            .flat_map(|secrets| secrets.values())
            .filter(|value| value.chars().count() >= MIN_MASKED_LEN)
            .flat_map(|value| escaped_forms(value))
            .collect();
        patterns.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        patterns.dedup();
        self.patterns = patterns;
    }
}

/// Never log while holding this lock: the logger reads it.
static LOG_MASK_SECRETS: Lazy<RwLock<LogMaskRegistry>> =
    Lazy::new(|| RwLock::new(LogMaskRegistry::default()));

fn update_log_secrets(change: impl FnOnce(&mut HashMap<String, SecretMap>)) {
    let mut registry = LOG_MASK_SECRETS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    change(&mut registry.sessions);
    registry.rebuild_patterns();
}

/// Makes the logger mask the session's current stored values.
pub fn register_log_secrets(session_id: &str, secrets: &SecretMap) {
    update_log_secrets(|sessions| {
        sessions.insert(session_id.to_string(), secrets.clone());
    });
}

/// Stops masking the session's values in logs. Call when its secrets are dropped.
pub fn forget_log_secrets(session_id: &str) {
    update_log_secrets(|sessions| {
        sessions.remove(session_id);
    });
}

/// The raw value plus the escaped copies a log line can carry: JSON string content
/// (webview lines forward tool arguments as JSON), `{:?}` content, and each of those
/// escaped once more (serialized args inside JSON, or a JSON string printed with `{:?}`).
fn escaped_forms(value: &str) -> Vec<String> {
    fn json_inner(text: &str) -> String {
        let quoted = serde_json::Value::String(text.to_string()).to_string();
        quoted[1..quoted.len() - 1].to_string()
    }
    fn debug_inner(text: &str) -> String {
        let quoted = format!("{text:?}");
        quoted[1..quoted.len() - 1].to_string()
    }
    let json = json_inner(value);
    let debug = debug_inner(value);
    vec![
        value.to_string(),
        json_inner(&json),
        debug_inner(&json),
        json_inner(&debug),
        debug_inner(&debug),
        json,
        debug,
    ]
}

/// Masks the stored values of every live session, raw or escaped, in a log line.
pub fn mask_log_line(line: &str) -> Cow<'_, str> {
    let registry = LOG_MASK_SECRETS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !registry
        .patterns
        .iter()
        .any(|pattern| line.contains(pattern.as_str()))
    {
        return Cow::Borrowed(line);
    }
    let mut masked = line.to_string();
    for pattern in &registry.patterns {
        if masked.contains(pattern.as_str()) {
            masked = masked.replace(pattern.as_str(), MASK);
        }
    }
    Cow::Owned(masked)
}

/// Shorter values are not masked: they match too much unrelated output.
const MIN_MASKED_LEN: usize = 4;
const MASK: &str = "***";

/// Names that change how the shell or an interpreter runs, or that the app sets itself.
const RESERVED_NAMES: &[&str] = &[
    "PATH",
    "BASH_ENV",
    "ENV",
    "SUDO_ASKPASS",
    "HOME",
    "PROMPT_COMMAND",
    "IFS",
    "SHELLOPTS",
    "BASHOPTS",
    "PS4",
    "SHELL",
    "NODE_OPTIONS",
    "PYTHONSTARTUP",
    "PYTHONPATH",
    "PYTHONUTF8",
    "PERL5OPT",
    "RUBYOPT",
    "GIT_SSH_COMMAND",
    crate::server::wake::WAKE_URL_ENV,
    crate::server::api_key::ENV_VAR,
    crate::server::api_key::ENV_FILE_VAR,
    "LIBRAGENT_API_KEY",
    "LIBRAGENT_BASE_URL",
    "LIBRAGENT_LLM_API_KEY",
    "LIBRAGENT_LLM_BASE_URL",
];

/// Checks a `storeAs` name. The error names the rule that failed.
pub fn validate_secret_name(name: &str) -> Result<(), String> {
    let well_formed = name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !well_formed {
        return Err(format!(
            "storeAs name '{name}' is not valid: it must match ^[A-Z][A-Z0-9_]*$"
        ));
    }
    if name.starts_with("LD_") || name.starts_with("DYLD_") {
        return Err(format!(
            "storeAs name '{name}' is reserved: names that start with LD_ or DYLD_ change how programs load"
        ));
    }
    // Docker shells also set secrets in the docker CLI's own environment.
    if name.starts_with("DOCKER_") {
        return Err(format!(
            "storeAs name '{name}' is reserved: names that start with DOCKER_ change which Docker daemon the shell uses"
        ));
    }
    if RESERVED_NAMES.contains(&name) || crate::utils::env::is_isolated_env_name(name) {
        return Err(format!(
            "storeAs name '{name}' is reserved: the app or the shell already uses this variable"
        ));
    }
    Ok(())
}

/// Replaces every stored value of `MIN_MASKED_LEN` or more characters with `***`.
pub fn mask_secrets(text: &str, secrets: &SecretMap) -> String {
    let mut values: Vec<&str> = secrets
        .values()
        .map(String::as_str)
        .filter(|value| value.chars().count() >= MIN_MASKED_LEN)
        .collect();
    // Longest first, so a value that contains another one is masked whole.
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));

    let mut masked = text.to_string();
    for value in values {
        if masked.contains(value) {
            masked = masked.replace(value, MASK);
        }
    }
    masked
}

/// Masks stored values in the text content and in every string of the structured content.
pub fn mask_result(mut result: MCPResult, secrets: &SecretMap) -> MCPResult {
    if secrets.is_empty() {
        return result;
    }
    for item in result.content.iter_mut().flatten() {
        if let MCPContent::Text { text } = item {
            *text = mask_secrets(text, secrets);
        }
    }
    if let Some(value) = result.structured_content.as_mut() {
        mask_json_strings(value, secrets);
    }
    result
}

/// Masks stored values in every string of a JSON value, at any depth.
pub fn mask_json_strings(value: &mut serde_json::Value, secrets: &SecretMap) {
    match value {
        serde_json::Value::String(text) => *text = mask_secrets(text, secrets),
        serde_json::Value::Array(items) => {
            for item in items {
                mask_json_strings(item, secrets);
            }
        }
        serde_json::Value::Object(fields) => {
            for field in fields.values_mut() {
                mask_json_strings(field, secrets);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_and_malformed_names_are_rejected() {
        for name in RESERVED_NAMES {
            assert!(
                validate_secret_name(name).is_err(),
                "{name} must be reserved"
            );
        }
        for name in [
            "LD_AUDIT",
            "DYLD_INSERT_LIBRARIES",
            "DOCKER_HOST",
            "DOCKER_CONTEXT",
            "DOCKER_CONFIG",
            "DOCKER_CERT_PATH",
            "DOCKER_TLS_VERIFY",
            "SHELLOPTS",
            "BASHOPTS",
            "PS4",
            "LIBRAGENT_WAKE_URL",
            "LANG",
            "LC_ALL",
            "GIT_DIR",
            "api_token",
            "1TOKEN",
            "",
        ] {
            assert!(
                validate_secret_name(name).is_err(),
                "{name} must be rejected"
            );
        }
    }

    #[test]
    fn ordinary_names_are_accepted() {
        for name in [
            "LIBRAGENT_EMAIL_PASSWORD",
            "LIBRAGENT_GOOGLE_CLIENT_SECRET",
            "TELEGRAM_CODE",
            "SUDO_PASSWORD",
        ] {
            assert_eq!(
                validate_secret_name(name),
                Ok(()),
                "{name} must be accepted"
            );
        }
    }

    #[test]
    fn values_of_four_or_more_characters_are_masked() {
        let secrets = SecretMap::from([
            ("SHORT".to_string(), "abc".to_string()),
            ("TOKEN".to_string(), "s3cr3t".to_string()),
            ("LONGER".to_string(), "s3cr3t-and-more".to_string()),
        ]);
        assert_eq!(
            mask_secrets("abc s3cr3t s3cr3t-and-more", &secrets),
            "abc *** ***"
        );
    }

    #[test]
    fn a_tool_result_is_masked_in_text_and_structured_content() {
        let secrets = SecretMap::from([("TOKEN".to_string(), "s3cr3t".to_string())]);
        let result = MCPResult {
            content: Some(vec![MCPContent::Text {
                text: "Output:\ns3cr3t".to_string(),
            }]),
            structured_content: Some(serde_json::json!({
                "stdout": "token=s3cr3t",
                "lines": ["s3cr3t"],
                "exit_code": 0
            })),
            is_error: None,
        };

        let masked = mask_result(result, &secrets);
        let serialized = serde_json::to_string(&masked).unwrap();
        assert!(!serialized.contains("s3cr3t"));
        assert_eq!(
            masked.structured_content,
            Some(serde_json::json!({
                "stdout": "token=***",
                "lines": ["***"],
                "exit_code": 0
            }))
        );
    }

    #[test]
    fn a_log_line_masks_stored_values_of_every_live_session_until_they_are_forgotten() {
        register_log_secrets(
            "log-mask-a",
            &SecretMap::from([("DEMO_TOKEN".to_string(), "logmask-a-7f3c".to_string())]),
        );
        register_log_secrets(
            "log-mask-b",
            &SecretMap::from([("OTHER_TOKEN".to_string(), "logmask-b-91d2".to_string())]),
        );
        let line = "echo val=$env:DEMO_TOKEN logmask-a-7f3c logmask-b-91d2 never-stored-8e1a";

        assert_eq!(
            mask_log_line(line),
            "echo val=$env:DEMO_TOKEN *** *** never-stored-8e1a"
        );

        forget_log_secrets("log-mask-a");
        forget_log_secrets("log-mask-b");
        assert_eq!(mask_log_line(line), line);
    }
}

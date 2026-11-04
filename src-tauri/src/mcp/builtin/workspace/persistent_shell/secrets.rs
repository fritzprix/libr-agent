//! Session-scoped shell secrets (#2544).
//!
//! A value the user gives once through a `storeAs` prompt is kept in memory for the
//! session and passed to the persistent shell as an environment variable. Values never
//! go into logs, tool results, or chat history; shell output is masked before it is
//! returned.

use std::collections::BTreeMap;

/// `name -> value` for one session.
pub type SecretMap = BTreeMap<String, String>;

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
}

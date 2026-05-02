use crate::session_isolation::IsolationLevel;
use regex::Regex;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Component, Path};

pub const INTERNAL_WORKSPACE_STATE_DIR: &str = ".libragent";
pub const INTERNAL_WORKSPACE_TMP_DIR: &str = "tmp";
pub const INTERNAL_WORKSPACE_EXPORTS_DIR: &str = "exports";

/// Get configured shell isolation level from settings
/// Returns the configured isolation level or Medium as default
pub async fn get_shell_isolation_level() -> IsolationLevel {
    use crate::repositories::settings_repository::SettingsRepository;
    use crate::state::get_settings_repository;

    let repo = get_settings_repository();

    // Get systemSettings object which contains shellIsolationLevel
    match repo.get("systemSettings").await {
        Ok(Some(model)) => {
            match serde_json::from_str::<Value>(&model.value) {
                Ok(json) => {
                    if let Some(level_str) =
                        json.get("shellIsolationLevel").and_then(|v| v.as_str())
                    {
                        match level_str {
                            "basic" => IsolationLevel::Basic,
                            "medium" => IsolationLevel::Medium,
                            "high" => IsolationLevel::High,
                            _ => IsolationLevel::Medium, // Invalid value, use default
                        }
                    } else {
                        IsolationLevel::Medium // Field not present, use default
                    }
                }
                Err(_) => IsolationLevel::Medium, // JSON parse error, use default
            }
        }
        _ => IsolationLevel::Medium, // Setting not found, use default
    }
}

/// Get diff context lines from settings (defaults to 3)
pub async fn get_diff_context_lines() -> usize {
    use crate::repositories::settings_repository::SettingsRepository;
    use crate::state::get_settings_repository;

    let repo = get_settings_repository();
    // Setting key is 'advancedSettings' based on RustSettingsService
    match repo.get("advancedSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<Value>(&model.value) {
            Ok(json) => json
                .get("diffContextLines")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(3),
            Err(_) => 3,
        },
        _ => 3,
    }
}

/// Validate timeout value, applying default and max limits
pub fn validate_timeout(timeout: Option<u64>) -> u64 {
    let default = crate::config::default_execution_timeout();
    let max = crate::config::max_execution_timeout();
    timeout.unwrap_or(default).min(max)
}

/// Remove sensitive flags from command for logging
/// Shared utility used across workspace server components
///
/// # Security
/// This function sanitizes commands before storing them in logs or ProcessRegistry
/// to prevent revealing sensitive implementation details like stdin-based password transmission.
///
/// # Examples
/// ```rust
/// use tauri_mcp_agent_lib::mcp::builtin::workspace::utils::sanitize_command_for_logging;
///
/// let cmd = "sudo -S apt install vim";
/// let sanitized = sanitize_command_for_logging(cmd);
/// assert_eq!(sanitized, "sudo apt install vim");
/// ```
pub fn sanitize_command_for_logging(command: &str) -> String {
    // Remove -S flag (read password from stdin) wherever it appears as a standalone flag
    // This is a simple regex replacement and does not respect quotes, which is acceptable for logging sanitization
    // (better to over-sanitize than leak security details)
    let re = Regex::new(r"(^|\s)-S\b").unwrap();
    let sanitized = re.replace_all(command, "$1").to_string();

    // Clean up any double spaces created by removal
    let re_spaces = Regex::new(r"\s+").unwrap();
    let sanitized = re_spaces.replace_all(&sanitized, " ").to_string();
    let sanitized = sanitized.trim().to_string();

    // Truncate if too long (safe string slicing)
    crate::utils::truncate_chars(&sanitized, 100)
}

pub fn is_internal_workspace_artifact_path(workspace_root: &Path, path: &Path) -> bool {
    let Ok(relative_path) = path.strip_prefix(workspace_root) else {
        return false;
    };

    let mut components = relative_path.components();
    let first = components.next();
    let second = components.next();

    // Check if path is literally inside .libragent/tmp or .libragent/exports
    if matches!(
        (&first, &second),
        (
            Some(Component::Normal(f)),
            Some(Component::Normal(s))
        ) if *f == OsStr::new(INTERNAL_WORKSPACE_STATE_DIR)
            && (*s == OsStr::new(INTERNAL_WORKSPACE_TMP_DIR)
                || *s == OsStr::new(INTERNAL_WORKSPACE_EXPORTS_DIR))
    ) {
        return true;
    }

    matches!(
        (&first, &second),
        (
            Some(Component::Normal(f)),
            Some(Component::Normal(s))
        ) if *f == OsStr::new(INTERNAL_WORKSPACE_STATE_DIR)
            && (*s == OsStr::new(INTERNAL_WORKSPACE_TMP_DIR)
                || *s == OsStr::new(INTERNAL_WORKSPACE_EXPORTS_DIR))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sanitize_sudo_s_flag() {
        assert_eq!(
            sanitize_command_for_logging("sudo -S apt install vim"),
            "sudo apt install vim"
        );

        // Handle -S in different positions
        assert_eq!(
            sanitize_command_for_logging("sudo -p 'prompt' -S apt update"),
            "sudo -p 'prompt' apt update"
        );
    }

    #[test]
    fn test_sanitize_no_change_needed() {
        assert_eq!(
            sanitize_command_for_logging("sudo apt install vim"),
            "sudo apt install vim"
        );

        assert_eq!(sanitize_command_for_logging("ls -la"), "ls -la");
    }

    #[test]
    fn test_truncate_long_commands() {
        let long_cmd = "a".repeat(150);
        let result = sanitize_command_for_logging(&long_cmd);
        assert_eq!(result.len(), 103); // 100 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_sanitize_multiple_sudo_occurrences() {
        // Note: Our simple sanitizer removes -S everywhere, even inside quotes.
        // This is acceptable for logging purposes.
        assert_eq!(
            sanitize_command_for_logging("sudo -S echo 'sudo -S test'"),
            "sudo echo 'sudo test'"
        );
    }

    #[test]
    fn internal_workspace_artifact_detection_is_precise() {
        let workspace_root = PathBuf::from("/workspace");

        assert!(is_internal_workspace_artifact_path(
            &workspace_root,
            &workspace_root.join(".libragent/tmp/process_123/stdout"),
        ));
        assert!(is_internal_workspace_artifact_path(
            &workspace_root,
            &workspace_root.join(".libragent/exports/packages/export.zip"),
        ));
        assert!(!is_internal_workspace_artifact_path(
            &workspace_root,
            &workspace_root.join(".libragent/tool-results/output.txt"),
        ));
        assert!(!is_internal_workspace_artifact_path(
            &workspace_root,
            &workspace_root.join(".libragent/teamwork.json"),
        ));
        assert!(!is_internal_workspace_artifact_path(
            &workspace_root,
            &workspace_root.join("tmp/process_123/stdout"),
        ));
    }
}

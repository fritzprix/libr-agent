use regex::Regex;

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
/// ```
/// use tauri_mcp_agent_lib::mcp::builtin::workspace::utils::sanitize_command_for_logging;
///
/// let cmd = "sudo -S apt install vim";
/// let sanitized = sanitize_command_for_logging(cmd);
/// assert_eq!(sanitized, "sudo apt install vim");
/// ```
pub fn sanitize_command_for_logging(command: &str) -> String {
    // Remove sudo -S flag using regex (handles various positions)
    let re = Regex::new(r"\bsudo\s+-S\b").unwrap();
    let sanitized = re.replace_all(command, "sudo").to_string();

    // Truncate if too long
    if sanitized.len() > 100 {
        format!("{}...", &sanitized[..100])
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            sanitize_command_for_logging("sudo -S echo 'sudo -S test'"),
            "sudo echo 'sudo -S test'"
        );
    }
}

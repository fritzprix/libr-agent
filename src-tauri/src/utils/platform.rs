#[cfg(target_os = "windows")]
pub const PLATFORM: &str = "windows";

#[cfg(target_os = "macos")]
pub const PLATFORM: &str = "macos";

#[cfg(target_os = "linux")]
pub const PLATFORM: &str = "linux";

pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Check if a command exists in PATH (cross-platform).
pub fn command_exists(cmd: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut command = std::process::Command::new("where");
        command.creation_flags(CREATE_NO_WINDOW);
        command.arg(cmd);

        command.env_clear();
        for (k, v) in crate::utils::env::get_isolated_env() {
            command.env(k, v);
        }

        command
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        let mut command = std::process::Command::new("sh");
        command.arg("-c");
        command.arg("command -v \"$1\"");
        command.arg("--");
        command.arg(cmd);

        command.env_clear();
        for (k, v) in crate::utils::env::get_isolated_env() {
            command.env(k, v);
        }

        command
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn test_command_exists_injection() {
        // Create a temporary file to verify no side effects occur
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("vuln_test_command_exists");
        let _ = std::fs::remove_file(&test_file);

        // Attempt command injection using ';'
        let malicious_cmd = format!("ls; touch {}", test_file.display());
        let result = command_exists(&malicious_cmd);

        assert!(!result);
        assert!(
            !test_file.exists(),
            "Command injection succeeded: side effect occurred"
        );

        // Attempt command injection using '$()'
        let malicious_cmd2 = format!("ls $(touch {})", test_file.display());
        let result2 = command_exists(&malicious_cmd2);

        assert!(!result2);
        assert!(
            !test_file.exists(),
            "Command injection succeeded: side effect occurred"
        );

        // Clean up
        let _ = std::fs::remove_file(test_file);
    }
}

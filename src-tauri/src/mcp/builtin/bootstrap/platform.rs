use serde::{Deserialize, Serialize};
use std::env;

/// Platform information for the current system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub shell: String,
    pub home_dir: Option<String>,
    pub temp_dir: String,
}

/// Detect the current platform information
///
/// Returns detailed information about the operating system, architecture,
/// shell environment, and key directory paths.
pub fn detect_current_platform() -> PlatformInfo {
    let os = detect_os();
    let arch = detect_arch();
    let shell = detect_shell(&os);
    let home_dir = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();
    let temp_dir = env::temp_dir().to_string_lossy().to_string();

    PlatformInfo {
        os,
        arch,
        shell,
        home_dir,
        temp_dir,
    }
}

/// Detect operating system
fn detect_os() -> String {
    match env::consts::OS {
        "windows" => "windows".to_string(),
        "macos" => "darwin".to_string(),
        "linux" => "linux".to_string(),
        other => other.to_string(),
    }
}

/// Detect CPU architecture
fn detect_arch() -> String {
    match env::consts::ARCH {
        "x86_64" => "x64".to_string(),
        "x86" => "x86".to_string(),
        "aarch64" => "arm64".to_string(),
        "arm" => "arm".to_string(),
        other => other.to_string(),
    }
}

/// Detect default shell for the platform
fn detect_shell(os: &str) -> String {
    match os {
        "windows" => {
            // Check for PowerShell vs CMD
            if env::var("PSModulePath").is_ok() {
                "powershell".to_string()
            } else {
                "cmd".to_string()
            }
        }
        "darwin" | "linux" => {
            // Check SHELL environment variable
            env::var("SHELL")
                .ok()
                .and_then(|shell_path| shell_path.split('/').next_back().map(|s| s.to_string()))
                .unwrap_or_else(|| "bash".to_string())
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform() {
        let platform = detect_current_platform();

        // Verify that we get non-empty values
        assert!(!platform.os.is_empty());
        assert!(!platform.arch.is_empty());
        assert!(!platform.shell.is_empty());
        assert!(!platform.temp_dir.is_empty());

        // OS should be one of the known values
        assert!(platform.os == "windows" || platform.os == "darwin" || platform.os == "linux");
    }

    #[test]
    fn test_detect_os() {
        let os = detect_os();
        assert!(!os.is_empty());
    }

    #[test]
    fn test_detect_arch() {
        let arch = detect_arch();
        assert!(!arch.is_empty());
    }

    #[test]
    fn test_detect_shell() {
        let os = detect_os();
        let shell = detect_shell(&os);
        assert!(!shell.is_empty());
    }
}

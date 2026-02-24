//! # Platform Detection Module
//!
//! This module provides comprehensive, cross-platform detection of system information.
//!
//! ## Supported Platforms
//! - **Windows**: Full support via `where` command and PowerShell detection
//! - **macOS**: Full support via `command -v` and Homebrew detection
//! - **Linux**: Full support with distro detection via `/etc/os-release`
//!
//! ## Features
//! - OS and architecture detection (all platforms)
//! - Shell detection (PowerShell/CMD on Windows, bash/zsh/fish on Unix)
//! - Linux distribution detection (Debian, Ubuntu, Arch, Fedora, etc.)
//! - Package manager detection (apt, brew, chocolatey, pacman, etc.)
//! - Installed development tools scanning (node, python, docker, git, cargo, etc.)
//! - Tool version detection and path resolution
//!
//! ## Cross-Platform Implementation
//! - Uses platform-specific commands for tool detection:
//!   - Windows: `where <tool>`
//!   - Unix: `command -v <tool>`
//! - Gracefully handles missing tools and features on each platform

use crate::utils::platform::command_exists;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Platform information for the current system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub shell: String,
    pub home_dir: Option<String>,
    pub temp_dir: String,
    pub distro: Option<LinuxDistro>,
    pub package_manager: Option<String>,
    pub installed_tools: HashMap<String, ToolInfo>,
}

/// Linux distribution information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDistro {
    pub name: String,
    pub id: String,
    pub version: Option<String>,
    pub pretty_name: Option<String>,
}

/// Information about an installed development tool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

/// Detect the current platform information
///
/// Returns detailed information about the operating system, architecture,
/// shell environment, key directory paths, Linux distribution, package manager,
/// and installed development tools.
pub fn detect_current_platform() -> PlatformInfo {
    let os = detect_os();
    let arch = detect_arch();
    let shell = detect_shell(&os);
    let home_dir = env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok();
    let temp_dir = env::temp_dir().to_string_lossy().to_string();
    let distro = if os == "linux" {
        detect_linux_distro()
    } else {
        None
    };
    let package_manager = detect_package_manager(&os);
    let installed_tools = detect_installed_tools();

    PlatformInfo {
        os,
        arch,
        shell,
        home_dir,
        temp_dir,
        distro,
        package_manager,
        installed_tools,
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

/// Detect Linux distribution from /etc/os-release
fn detect_linux_distro() -> Option<LinuxDistro> {
    let os_release = std::fs::read_to_string("/etc/os-release").ok()?;

    let mut name = String::new();
    let mut id = String::new();
    let mut version = None;
    let mut pretty_name = None;

    for line in os_release.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim_matches('"');
            match key {
                "NAME" => name = value.to_string(),
                "ID" => id = value.to_string(),
                "VERSION_ID" => version = Some(value.to_string()),
                "PRETTY_NAME" => pretty_name = Some(value.to_string()),
                _ => {}
            }
        }
    }

    if !id.is_empty() {
        Some(LinuxDistro {
            name,
            id,
            version,
            pretty_name,
        })
    } else {
        None
    }
}

/// Detect available package manager
fn detect_package_manager(os: &str) -> Option<String> {
    let package_managers = match os {
        "linux" => vec!["apt", "apt-get", "dnf", "yum", "pacman", "zypper", "apk"],
        "darwin" => vec!["brew", "port"],
        "windows" => vec!["choco", "scoop", "winget"],
        _ => return None,
    };

    for pm in package_managers {
        if command_exists(pm) {
            return Some(pm.to_string());
        }
    }

    None
}

/// Detect installed development tools
fn detect_installed_tools() -> HashMap<String, ToolInfo> {
    let tools = vec![
        "node", "npm", "python", "python3", "pip", "pip3", "uv", "docker", "git", "cargo", "rustc",
        "go", "java", "gcc", "make",
    ];

    let mut results = HashMap::new();

    for tool in tools {
        let info = if command_exists(tool) {
            let version = get_tool_version(tool);
            let path = get_command_path(tool);
            ToolInfo {
                installed: true,
                version,
                path,
            }
        } else {
            ToolInfo {
                installed: false,
                version: None,
                path: None,
            }
        };

        results.insert(tool.to_string(), info);
    }

    results
}

/// Check if a command exists in PATH (cross-platform)
/// Get the version of a tool
fn get_tool_version(tool: &str) -> Option<String> {
    #[cfg(windows)]
    let output = {
        use std::os::windows::process::CommandExt;
        Command::new(tool)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("--version")
            .output()
            .ok()?
    };
    #[cfg(not(windows))]
    let output = Command::new(tool).arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Extract first line as version
    combined
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the full path of a command (cross-platform)
fn get_command_path(cmd: &str) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Windows: Use 'where' command (returns first match)
        let output = Command::new("where")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(cmd)
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .and_then(|s| s.lines().next().map(|line| line.trim().to_string()))
                .filter(|s| !s.is_empty())
        } else {
            None
        }
    }

    #[cfg(not(windows))]
    {
        // Unix-like: Use 'command -v'
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {}", cmd))
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        }
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

        // Verify Linux distro detection on Linux only
        #[cfg(target_os = "linux")]
        {
            assert!(platform.distro.is_some(), "Linux distro should be detected");
        }

        // Verify package manager detection
        assert!(
            platform.package_manager.is_some(),
            "Package manager should be detected on most systems"
        );

        // Verify installed tools map is populated
        assert!(
            !platform.installed_tools.is_empty(),
            "Installed tools should be checked"
        );
    }

    #[test]
    fn test_detect_os() {
        let os = detect_os();
        assert!(!os.is_empty());

        // Verify OS-specific detection
        #[cfg(target_os = "windows")]
        assert_eq!(os, "windows");

        #[cfg(target_os = "macos")]
        assert_eq!(os, "darwin");

        #[cfg(target_os = "linux")]
        assert_eq!(os, "linux");
    }

    #[test]
    fn test_detect_arch() {
        let arch = detect_arch();
        assert!(!arch.is_empty());
        // Should be one of: x64, x86, arm64, arm
        assert!(
            arch == "x64" || arch == "x86" || arch == "arm64" || arch == "arm",
            "Architecture should be normalized to common values"
        );
    }

    #[test]
    fn test_detect_shell() {
        let os = detect_os();
        let shell = detect_shell(&os);
        assert!(!shell.is_empty());

        // Verify shell detection per platform
        #[cfg(target_os = "windows")]
        assert!(
            shell == "powershell" || shell == "cmd",
            "Windows should detect PowerShell or CMD"
        );

        #[cfg(not(target_os = "windows"))]
        {
            // Unix-like systems should detect common shells
            let valid_shells = ["bash", "zsh", "fish", "sh", "tcsh", "ksh"];
            assert!(
                valid_shells.contains(&shell.as_str()),
                "Unix shell should be one of the common shells, got: {}",
                shell
            );
        }
    }

    #[test]
    fn test_package_manager_detection() {
        let os = detect_os();
        let pm = detect_package_manager(&os);

        // Most development systems have a package manager
        // This test documents expected behavior per platform
        match os.as_str() {
            "windows" => {
                // Windows may or may not have choco/scoop/winget installed
                if let Some(manager) = pm {
                    assert!(
                        manager == "choco" || manager == "scoop" || manager == "winget",
                        "Windows package manager should be choco, scoop, or winget"
                    );
                }
            }
            "darwin" => {
                // macOS commonly has Homebrew
                if let Some(manager) = pm {
                    assert!(
                        manager == "brew" || manager == "port",
                        "macOS package manager should be brew or port"
                    );
                }
            }
            "linux" => {
                // Linux almost always has a package manager
                if let Some(manager) = pm {
                    let valid_managers =
                        ["apt", "apt-get", "dnf", "yum", "pacman", "zypper", "apk"];
                    assert!(
                        valid_managers.contains(&manager.as_str()),
                        "Linux package manager should be one of the common ones, got: {}",
                        manager
                    );
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_command_exists_cross_platform() {
        // Test with a command that should exist on all platforms
        // Note: This test assumes the system has basic shell commands

        #[cfg(target_os = "windows")]
        {
            // Windows always has 'where' command
            assert!(
                command_exists("where"),
                "'where' command should exist on Windows"
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Unix-like systems always have 'sh' and 'ls'
            assert!(
                command_exists("sh"),
                "'sh' should exist on Unix-like systems"
            );
            assert!(
                command_exists("ls"),
                "'ls' should exist on Unix-like systems"
            );
        }

        // Test with a command that definitely doesn't exist
        assert!(
            !command_exists("this_command_definitely_does_not_exist_12345"),
            "Non-existent command should return false"
        );
    }
}

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
        std::process::Command::new("where")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v \"$1\"")
            .arg("--")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

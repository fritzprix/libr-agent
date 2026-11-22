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

/// Windows creation flag that prevents a console window from appearing for GUI hosts.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Suppress console window flashing when spawning a child process (Windows only).
pub fn suppress_console_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

/// Async variant of [`suppress_console_window`] for `tokio::process::Command`.
pub fn suppress_console_window_async(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        // tokio::process::Command exposes creation_flags on Windows without CommandExt.
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

/// Check if a command exists in PATH (cross-platform).
pub fn command_exists(cmd: &str) -> bool {
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("where");
        suppress_console_window(&mut command);
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

/// Whether to set `WEBKIT_DISABLE_DMABUF_RENDERER=1` as the WebKitGTK +
/// NVIDIA proprietary-driver workaround (#2964): only when the NVIDIA driver
/// is detected and the user has not set the variable themselves. An existing
/// user value (including `0`) is always kept.
pub fn should_set_dmabuf_workaround(
    nvidia_detected: bool,
    user_value: Option<&std::ffi::OsStr>,
) -> bool {
    nvidia_detected && user_value.is_none()
}

/// True when the NVIDIA proprietary driver looks present: `/proc/driver/nvidia`
/// exists, or the `nvidia_drm` module is loaded. Roots are parameters so the
/// unit tests can drive both markers with fixture directories.
#[cfg(target_os = "linux")]
fn nvidia_driver_present_in(proc_root: &std::path::Path, module_root: &std::path::Path) -> bool {
    proc_root.join("driver/nvidia").exists() || module_root.join("nvidia_drm").exists()
}

#[cfg(target_os = "linux")]
fn nvidia_driver_present() -> bool {
    nvidia_driver_present_in(
        std::path::Path::new("/proc"),
        std::path::Path::new("/sys/module"),
    )
}

/// Apply the WebKitGTK DMA-BUF workaround for the NVIDIA proprietary driver
/// (#2964). Must run before any WebView is created: without it, WebKitGTK's
/// watchdog can kill `WebKitWebProcess` and the renderer spins at 100% CPU.
///
/// Sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` only on NVIDIA systems and never
/// overwrites a user value. Trade-off: NVIDIA users get a slower render path
/// until an upstream fix lands.
#[cfg(target_os = "linux")]
pub fn apply_webkit_dmabuf_workaround() {
    const NAME: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if !should_set_dmabuf_workaround(nvidia_driver_present(), std::env::var_os(NAME).as_deref()) {
        return;
    }
    std::env::set_var(NAME, "1");
    log::info!(
        "NVIDIA driver detected: set WEBKIT_DISABLE_DMABUF_RENDERER=1 (WebKitGTK DMA-BUF hang workaround, #2964)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn dmabuf_workaround_set_only_on_nvidia_without_user_value() {
        // NVIDIA + untouched variable: apply the workaround.
        assert!(should_set_dmabuf_workaround(true, None));
        // Non-NVIDIA: never touch the environment.
        assert!(!should_set_dmabuf_workaround(false, None));
        // Existing user values are kept as-is, including "0".
        assert!(!should_set_dmabuf_workaround(true, Some(OsStr::new("0"))));
        assert!(!should_set_dmabuf_workaround(true, Some(OsStr::new("1"))));
        assert!(!should_set_dmabuf_workaround(false, Some(OsStr::new("0"))));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn nvidia_detection_reads_proc_and_sys_module_markers() {
        let proc_root = tempfile::tempdir().unwrap();
        let module_root = tempfile::tempdir().unwrap();

        // Neither marker present.
        assert!(!nvidia_driver_present_in(
            proc_root.path(),
            module_root.path()
        ));

        // /proc/driver/nvidia present (driver loaded).
        std::fs::create_dir_all(proc_root.path().join("driver/nvidia")).unwrap();
        assert!(nvidia_driver_present_in(
            proc_root.path(),
            module_root.path()
        ));

        // Only /sys/module/nvidia_drm present (module loaded).
        std::fs::remove_dir_all(proc_root.path().join("driver")).unwrap();
        std::fs::create_dir_all(module_root.path().join("nvidia_drm")).unwrap();
        assert!(nvidia_driver_present_in(
            proc_root.path(),
            module_root.path()
        ));
    }

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

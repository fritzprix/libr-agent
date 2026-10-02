//! Detect whether a Chromium user-data-dir appears locked by a live process.
//!
//! Used before launching the sticky agent profile and before clearing it.

use std::path::Path;

#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

/// True when Chrome's profile SingletonLock points at a still-running process.
pub fn chrome_profile_appears_in_use(user_data_dir: &Path) -> bool {
    let lock_path = user_data_dir.join("SingletonLock");
    // Chrome's SingletonLock is often a symlink to "hostname-pid" (not a real path).
    // `Path::exists` follows links and would report false — use symlink_metadata instead.
    let Ok(meta) = std::fs::symlink_metadata(&lock_path) else {
        return false;
    };

    #[cfg(unix)]
    {
        if meta.file_type().is_symlink() {
            if let Ok(target) = std::fs::read_link(&lock_path) {
                if let Some(pid) = parse_chrome_singleton_pid(&target) {
                    return process_appears_alive(pid);
                }
            }
        }
        // Non-symlink lock (or unreadable target): treat as in-use to avoid a confusing CDP timeout.
        true
    }

    #[cfg(windows)]
    {
        let _ = meta;
        // Windows holds SingletonLock while Chrome is running; a live second instance fails fast.
        true
    }
}

// Unix SingletonLock is a `hostname-pid` symlink. Windows treats any live lock as
// in-use, so this parser is only needed for the Unix path and unit tests.
#[cfg(any(unix, test))]
fn parse_chrome_singleton_pid(target: impl AsRef<std::ffi::OsStr>) -> Option<u32> {
    let raw = target.as_ref().to_string_lossy();
    // Chrome writes "hostname-pid" (symlink target on Unix).
    let pid_str = raw.rsplit('-').next()?;
    pid_str.parse().ok()
}

#[cfg(target_os = "linux")]
fn process_appears_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(target_os = "macos")]
fn process_appears_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn parse_singleton_pid_from_chrome_lock_target() {
        assert_eq!(
            parse_chrome_singleton_pid(OsStr::new("myhost-12345")),
            Some(12345)
        );
        assert_eq!(parse_chrome_singleton_pid(OsStr::new("bad")), None);
    }

    #[test]
    fn profile_not_in_use_when_lock_missing() {
        let dir = std::env::temp_dir().join("libragent-chrome-attach-no-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        assert!(!chrome_profile_appears_in_use(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn profile_in_use_when_singleton_points_at_self() {
        let dir = std::env::temp_dir().join("libragent-chrome-attach-self-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let lock = dir.join("SingletonLock");
        let target = format!("testhost-{}", std::process::id());
        std::os::unix::fs::symlink(&target, &lock).expect("symlink");
        assert!(chrome_profile_appears_in_use(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

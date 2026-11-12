//! Data-dir single-instance lock.
//!
//! Two runtimes pointing at the same data dir would race on SQLite, session
//! state, and logs. The lock makes the second one fail at startup with a
//! clear message instead: stop the other instance, or start a second one
//! with its own `LIBRAGENT_DATA_DIR` (attach-style isolation).
//!
//! This also makes startup session recovery ownerless by construction:
//! `recover_sessions` only ever runs in the lock owner, so any `Busy` /
//! `Queued` row it finds belongs to a dead process, never to a live one.
//!
//! The lock file is created exclusively (`create_new`) and holds the owner's
//! PID. A lock whose PID is no longer alive (crash residue) is removed and
//! the acquire retried once. The file is deleted when the guard drops.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};

/// Lock file name inside the data directory.
pub const RUNTIME_LOCK_FILE: &str = ".runtime.lock";

/// Paths this process currently holds a `RuntimeLock` on. The lock file's
/// PID alone cannot tell a live holder from crash residue when the holder
/// *is* this process, so in-process re-acquires are answered from here.
static HELD_PATHS: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn held_paths() -> MutexGuard<'static, HashSet<PathBuf>> {
    HELD_PATHS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Held for the whole runtime lifetime; removing it releases the data dir.
#[derive(Debug)]
pub struct RuntimeLock {
    path: PathBuf,
}

impl Drop for RuntimeLock {
    fn drop(&mut self) {
        held_paths().remove(&self.path);
        let _ = fs::remove_file(&self.path);
    }
}

impl RuntimeLock {
    /// Acquire `<data_dir>/.runtime.lock` for this process.
    ///
    /// Fails with an actionable message when another live runtime holds the
    /// data dir. Stale locks (dead PID) are cleaned up and retried once.
    pub fn acquire(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data dir {}: {e}", data_dir.display()))?;
        let path = data_dir.join(RUNTIME_LOCK_FILE);

        // Our own PID sits in the lock file while we hold it, so the stale-PID
        // logic below would accept a second acquire from this very process.
        if held_paths().contains(&path) {
            return Err(format!(
                "Another LibrAgent runtime (this process) holds this data dir: {}. \
                 Stop that instance, or run this one with a separate LIBRAGENT_DATA_DIR.",
                data_dir.display()
            ));
        }

        for attempt in 0..2 {
            match Self::write_lock_file(&path) {
                Ok(()) => {
                    held_paths().insert(path.clone());
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    match Self::stale_lock_action(&path) {
                        StaleAction::Blocked { holder } => {
                            return Err(format!(
                                "Another LibrAgent runtime (PID {holder}) holds this data dir: {}. \
                                 Stop that instance, or run this one with a separate LIBRAGENT_DATA_DIR.",
                                data_dir.display()
                            ));
                        }
                        StaleAction::RemoveStale => {
                            if attempt == 1 {
                                return Err(format!(
                                    "Failed to clear stale runtime lock {}: retry startup",
                                    path.display()
                                ));
                            }
                            log::warn!(
                                "Removing stale runtime lock {} (holder PID is not alive)",
                                path.display()
                            );
                            fs::remove_file(&path).map_err(|e| {
                                format!(
                                    "Failed to remove stale runtime lock {}: {e}",
                                    path.display()
                                )
                            })?;
                        }
                    }
                }
                Err(error) => {
                    return Err(format!(
                        "Failed to acquire runtime lock {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        unreachable!("acquire loop returns on every path")
    }

    /// Write `"<pid>\n"` exclusively. `AlreadyExists` means someone else holds it.
    fn write_lock_file(path: &Path) -> std::io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        writeln!(file, "{}", std::process::id())
    }

    /// Decide what to do with an existing lock file.
    fn stale_lock_action(path: &Path) -> StaleAction {
        let Ok(raw) = fs::read_to_string(path) else {
            // Unreadable: treat as stale so a permissions hiccup never bricks startup.
            return StaleAction::RemoveStale;
        };
        let Some(pid) = raw
            .lines()
            .next()
            .and_then(|line| line.trim().parse::<u32>().ok())
        else {
            // Unparseable: treat as stale rather than blocking startup forever.
            return StaleAction::RemoveStale;
        };
        if pid == std::process::id() {
            // Our own leftover (e.g. restart in the same PID); safe to replace.
            return StaleAction::RemoveStale;
        }
        if pid_is_alive(pid) {
            StaleAction::Blocked { holder: pid }
        } else {
            StaleAction::RemoveStale
        }
    }
}

enum StaleAction {
    Blocked { holder: u32 },
    RemoveStale,
}

/// True when `pid` currently exists on this machine.
fn pid_is_alive(pid: u32) -> bool {
    // `0` is not a process, and values above `i32::MAX` reach process-group
    // semantics (`kill(-1, …)` signals everything) instead of one process.
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    #[cfg(unix)]
    {
        // Signal 0 performs error checking without delivering a signal.
        // EPERM means the process exists but belongs to another user.
        let ok = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        ok || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut exit_code: u32 = 0;
            let queried = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);
            queried != 0 && exit_code == STILL_ACTIVE
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Unknown platform: assume alive so we never double-run on shared state.
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_blocks_second_holder_then_releases_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock = RuntimeLock::acquire(dir.path()).expect("first acquire should succeed");
        assert!(dir.path().join(RUNTIME_LOCK_FILE).exists());

        let second = RuntimeLock::acquire(dir.path());
        let err = second.expect_err("second acquire should fail while the lock is held");
        assert!(
            err.contains("Another LibrAgent runtime"),
            "unexpected: {err}"
        );

        drop(lock);
        assert!(!dir.path().join(RUNTIME_LOCK_FILE).exists());
        assert!(
            RuntimeLock::acquire(dir.path()).is_ok(),
            "re-acquire after drop"
        );
    }

    #[test]
    fn stale_lock_from_dead_pid_is_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(RUNTIME_LOCK_FILE);
        // PID outside any realistic range: guaranteed not alive on all platforms.
        fs::write(&path, "4294967295\n").expect("write stale lock");

        let lock = RuntimeLock::acquire(dir.path()).expect("stale lock should be replaced");
        let contents = fs::read_to_string(&path).expect("lock readable");
        assert_eq!(contents.trim(), std::process::id().to_string());
        drop(lock);
    }

    #[test]
    fn unparseable_lock_is_treated_as_stale() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(RUNTIME_LOCK_FILE);
        fs::write(&path, "not-a-pid").expect("write junk lock");
        assert!(RuntimeLock::acquire(dir.path()).is_ok());
    }
}

//! Exclusive startup lock over the backend descriptor namespace (#2757).
//!
//! Two frontends starting at the same moment can both see "no live backend"
//! and, unless the check-then-start sequence is serialized, each starts its
//! own HTTP server, scheduler, and session executor. Whoever wins the atomic
//! exclusive creation of the lock file runs the startup sequence; the loser
//! waits and then attaches to the backend the winner publishes.
//!
//! The lock file holds the holder's pid so a lock left behind by a dead
//! process can be swept. Following `backend_descriptor`'s record probes, a
//! failed probe never sweeps: death must be proven, not assumed. A stale
//! sweep is atomic — the winner renames the file aside, losers see the file
//! gone and retry.
//!
//! Dropping a [`StartupLock`] removes the file only while its content still
//! names this process, so a swept holder never deletes a fresh lock.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};

/// Recheck interval while a lock is held by someone else.
pub const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Outcome of one non-blocking acquisition attempt.
#[derive(Debug)]
pub enum LockAttempt {
    /// The lock file was created by this attempt; this process may start.
    Acquired(StartupLock),
    /// The lock exists and its holder is (or might be) alive; retry later.
    Held,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LockOwner {
    pid: u32,
    acquired_at_ms: i64,
}

/// A held exclusive startup lock. Released (file removed) on drop.
#[derive(Debug)]
pub struct StartupLock {
    path: std::path::PathBuf,
    /// The pid written into the lock file at acquisition.
    holder_pid: u32,
}

impl Drop for StartupLock {
    fn drop(&mut self) {
        // Remove only our own lock: if a sweep replaced the file while we
        // believed we held it, the new owner's file must survive. The
        // comparison is against the pid this guard acquired with, not the
        // current process id, so injected-pid holders release correctly.
        let still_ours = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str::<LockOwner>(&raw).ok())
            .is_some_and(|owner| owner.pid == self.holder_pid);
        if still_ours {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// One acquisition attempt with the process probes in use for this lock.
pub fn try_acquire(path: &Path) -> LockAttempt {
    try_acquire_with_probes(
        path,
        std::process::id(),
        crate::utils::process::process_is_alive,
    )
}

/// One acquisition attempt with injectable probes (tests).
///
/// `pid_probe` follows `backend_descriptor`'s convention: `Some(true)`
/// alive, `Some(false)` definitively dead, `None` the probe itself failed
/// (never sweeps). A lock naming this very process is crash residue from a
/// released pid and is swept even though the probe would call it alive.
pub fn try_acquire_with_probes(
    path: &Path,
    own_pid: u32,
    pid_probe: impl Fn(u32) -> std::io::Result<bool>,
) -> LockAttempt {
    let body = serde_json::to_string(&LockOwner {
        pid: own_pid,
        acquired_at_ms: chrono::Utc::now().timestamp_millis(),
    })
    .expect("serializing lock owner");

    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            if file.write_all(body.as_bytes()).is_ok() {
                return LockAttempt::Acquired(StartupLock {
                    path: path.to_path_buf(),
                    holder_pid: own_pid,
                });
            }
            // A failed write leaves the half-open file behind; drop it so a
            // later attempt sees a clean namespace, and report Held.
            drop(file);
            let _ = std::fs::remove_file(path);
            LockAttempt::Held
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let owner = std::fs::read_to_string(path)
                .ok()
                .and_then(|raw| serde_json::from_str::<LockOwner>(&raw).ok());
            let stale = match owner {
                // Unreadable or malformed content: proven unusable, sweep.
                None => true,
                Some(owner) => {
                    if owner.pid == own_pid {
                        // Our own pid cannot hold this lock in this run:
                        // residue of a dead process whose pid the OS reused.
                        true
                    } else {
                        pid_probe(owner.pid).ok() == Some(false)
                    }
                }
            };
            if stale {
                // Atomic sweep: whoever renames the stale file aside wins;
                // losers see the rename fail and simply retry.
                if sweep_aside(path) {
                    return LockAttempt::Held; // file is gone; retry now
                }
            }
            LockAttempt::Held
        }
        Err(_) => LockAttempt::Held,
    }
}

/// Rename a stale lock aside exactly once among racing sweepers, then delete.
fn sweep_aside(path: &Path) -> bool {
    let mut trash_name = match path.file_name() {
        Some(name) => name.to_os_string(),
        None => return false,
    };
    trash_name.push(format!(".stale.{}", std::process::id()));
    let trash = path.with_file_name(trash_name);
    let _ = std::fs::remove_file(&trash);
    // Only one racing process can rename a still-existing file.
    if std::fs::rename(path, &trash).is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&trash);
    true
}

/// Wait until the lock is free, then acquire it, or give up at `deadline`.
pub async fn acquire(path: &Path, deadline: Instant) -> Result<StartupLock, String> {
    loop {
        match try_acquire(path) {
            LockAttempt::Acquired(lock) => return Ok(lock),
            LockAttempt::Held => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "Another frontend holds the backend startup lock at {} and no live backend appeared",
                        path.display()
                    ));
                }
                tokio::time::sleep(LOCK_POLL_INTERVAL).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn alive(_pid: u32) -> std::io::Result<bool> {
        Ok(true)
    }

    fn dead(_pid: u32) -> std::io::Result<bool> {
        Ok(false)
    }

    fn write_lock(path: &Path, pid: u32) {
        let body = serde_json::to_string(&LockOwner {
            pid,
            acquired_at_ms: 1,
        })
        .unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn first_creator_acquires_and_drop_releases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");

        let lock = match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Held => panic!("first attempt must acquire"),
        };
        assert!(path.exists());

        match try_acquire_with_probes(&path, 222, alive) {
            LockAttempt::Acquired(_) => panic!("second attempt must not acquire while held"),
            LockAttempt::Held => {}
        }

        drop(lock);
        assert!(!path.exists(), "drop must release the lock");
    }

    #[test]
    fn held_lock_by_live_pid_is_not_swept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");
        write_lock(&path, 999_999);

        match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(_) => panic!("a live holder's lock must not be swept"),
            LockAttempt::Held => {}
        }
        assert!(path.exists());
    }

    #[test]
    fn probe_failure_never_sweeps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");
        write_lock(&path, 999_999);

        let probe_fails = |_pid: u32| Err(std::io::Error::other("probe failed"));
        match try_acquire_with_probes(&path, 111, probe_fails) {
            LockAttempt::Acquired(_) => panic!("an unverifiable holder must not be swept"),
            LockAttempt::Held => {}
        }
        assert!(path.exists());
    }

    #[test]
    fn dead_holder_is_swept_and_next_attempt_acquires() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");
        write_lock(&path, 999_999);

        match try_acquire_with_probes(&path, 111, dead) {
            LockAttempt::Acquired(_) => panic!("first attempt sees the stale lock; sweep first"),
            LockAttempt::Held => {}
        }
        match try_acquire_with_probes(&path, 111, dead) {
            LockAttempt::Acquired(_) => {}
            LockAttempt::Held => panic!("after sweeping a dead holder, acquire must succeed"),
        }
    }

    #[test]
    fn own_pid_residue_is_swept_even_though_the_pid_is_alive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");
        write_lock(&path, 111);

        match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(_) => panic!("residue must be swept first"),
            LockAttempt::Held => {}
        }
        match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(_) => {}
            LockAttempt::Held => panic!("after sweeping own residue, acquire must succeed"),
        }
    }

    #[test]
    fn malformed_lock_content_is_swept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.backend.lock");
        std::fs::write(&path, "not json").unwrap();

        match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(_) => panic!("first attempt sweeps"),
            LockAttempt::Held => {}
        }
        match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(_) => {}
            LockAttempt::Held => panic!("malformed lock must be sweepable"),
        }
    }

    #[test]
    fn drop_never_deletes_another_processes_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = PathBuf::from(dir.path()).join("db.backend.lock");

        let lock = match try_acquire_with_probes(&path, 111, alive) {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Held => panic!("first attempt must acquire"),
        };
        // Simulate a sweep-and-replace by another process while we "hold".
        std::fs::remove_file(&path).unwrap();
        write_lock(&path, 222);

        drop(lock);
        let survivor = std::fs::read_to_string(&path).unwrap();
        assert!(survivor.contains("222"), "a fresh lock must survive");
    }
}

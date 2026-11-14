//! Exclusive startup lock over the backend descriptor namespace (#2757).
//!
//! Two frontends starting at the same moment can both see "no live backend"
//! and, unless the check-then-start sequence is serialized, each starts its
//! own HTTP server, scheduler, and session executor. Whoever holds the
//! exclusive OS advisory lock on the lock file runs the startup sequence;
//! the loser waits and then attaches to the backend the winner publishes.
//!
//! The lock is the OS lock on an open file handle (`flock` on unix,
//! `LockFileEx` on Windows), not the file's existence or content:
//! - The OS releases it when the holder's process dies, so a crash leaves
//!   nothing to sweep and a leftover file never blocks anyone.
//! - The file is never deleted, so a lock can never be removed from under a
//!   live holder.
//! - The file content is diagnostic only (who held the lock last) and never
//!   participates in acquisition, so half-written content misleads nobody.

use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::{Duration, Instant};

/// Recheck interval while a lock is held by someone else.
pub const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Outcome of one non-blocking acquisition attempt.
#[derive(Debug)]
pub enum LockAttempt {
    /// This attempt holds the OS lock; this process may start.
    Acquired(StartupLock),
    /// Another holder has the OS lock; retry later.
    Held,
    /// A real I/O error occurred; retrying will not help.
    Failed(String),
}

/// A held exclusive startup lock. The OS releases the lock when the guard
/// is dropped or the process dies; the lock file itself stays in place.
#[derive(Debug)]
pub struct StartupLock {
    /// Held open purely to keep the OS lock alive: closing this handle
    /// releases the lock, so the field is never read.
    #[allow(dead_code)]
    file: File,
}

/// Why one non-blocking lock syscall refused.
enum LockError {
    /// Another handle holds the lock.
    Busy,
    /// Any other I/O error.
    Io(std::io::Error),
}

/// One acquisition attempt: open the lock file, then take the OS lock on
/// the handle without blocking.
pub fn try_acquire(path: &Path) -> LockAttempt {
    let file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
    {
        Ok(file) => file,
        Err(err) => {
            return LockAttempt::Failed(format!(
                "cannot open backend startup lock at {}: {err}",
                path.display()
            ));
        }
    };
    match try_lock_os(&file) {
        Ok(()) => {
            // Diagnostic only: content never feeds the acquisition decision,
            // so a concurrent reader cannot be misled and nothing sweeps.
            let _ = write_owner_note(&file);
            LockAttempt::Acquired(StartupLock { file })
        }
        Err(LockError::Busy) => LockAttempt::Held,
        Err(LockError::Io(err)) => LockAttempt::Failed(format!(
            "cannot lock backend startup lock at {}: {err}",
            path.display()
        )),
    }
}

/// Record the current holder for debugging. Best-effort: only this process
/// holds the OS lock, so no other holder writes concurrently.
fn write_owner_note(file: &File) -> std::io::Result<()> {
    use std::io::Write;
    let note = format!(
        "pid={} acquired_at_ms={}\n",
        std::process::id(),
        chrono::Utc::now().timestamp_millis(),
    );
    file.set_len(0)?;
    (&*file).write_all(note.as_bytes())
}

#[cfg(unix)]
fn try_lock_os(file: &File) -> Result<(), LockError> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: `file` keeps the fd valid for the call; LOCK_NB makes flock
    // return instead of blocking.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Err(LockError::Busy)
    } else {
        Err(LockError::Io(err))
    }
}

#[cfg(windows)]
fn try_lock_os(file: &File) -> Result<(), LockError> {
    use std::mem;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_IO_PENDING, ERROR_LOCK_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    // SAFETY: zeroed OVERLAPPED is the documented input for a synchronous
    // handle; the byte range (1 byte at offset 0) is the protocol's marker
    // for "whole lock file held" and every caller locks the same range.
    let mut overlapped: OVERLAPPED = unsafe { mem::zeroed() };
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if ok != 0 {
        return Ok(());
    }
    // SAFETY: GetLastError reads this thread's last error right after the
    // failed call above.
    let code = unsafe { GetLastError() };
    if code == ERROR_LOCK_VIOLATION || code == ERROR_IO_PENDING {
        Err(LockError::Busy)
    } else {
        Err(LockError::Io(std::io::Error::from_raw_os_error(
            code as i32,
        )))
    }
}

/// Wait until the lock is free, then acquire it, or give up at `deadline`.
/// A [`LockAttempt::Failed`] error surfaces immediately rather than
/// burning the whole timeout on a problem retrying cannot fix.
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
            LockAttempt::Failed(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("db.backend.lock")
    }

    fn acquire_or_panic(path: &Path) -> StartupLock {
        match try_acquire(path) {
            LockAttempt::Acquired(lock) => lock,
            other => panic!("first attempt must acquire, got {other:?}"),
        }
    }

    #[test]
    fn holder_blocks_others_and_drop_releases_without_deleting_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = lock_path(&dir);

        let lock = acquire_or_panic(&path);
        assert!(path.exists());

        match try_acquire(&path) {
            LockAttempt::Acquired(_) => panic!("second attempt must not acquire while held"),
            LockAttempt::Held => {}
            LockAttempt::Failed(err) => panic!("unexpected failure while held: {err}"),
        }

        drop(lock);
        assert!(
            path.exists(),
            "the lock file must stay: deleting it can race a waiting holder"
        );
        let _ = acquire_or_panic(&path);
    }

    #[test]
    fn leftover_lock_file_without_a_holder_is_acquired_immediately() {
        // A crashed or updated process leaves the file behind with no OS
        // lock on it. There is nothing to probe and nothing to sweep: the
        // lock is simply free.
        let dir = tempfile::tempdir().unwrap();
        let path = lock_path(&dir);
        std::fs::write(&path, "leftover from an old run").unwrap();

        let _lock = acquire_or_panic(&path);
    }

    #[test]
    fn unopenable_lock_path_fails_with_the_real_error() {
        // S1.1: a real I/O problem must not masquerade as `Held`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no-such-dir").join("db.backend.lock");

        match try_acquire(&path) {
            LockAttempt::Failed(err) => assert!(
                err.contains("cannot open backend startup lock"),
                "unexpected error: {err}"
            ),
            other => panic!("an unopenable path must fail with the real error, got {other:?}"),
        }
    }
}

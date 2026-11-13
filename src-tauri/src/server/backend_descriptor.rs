//! Per-runtime discovery records for running backends — not locks.
//!
//! Every backend process writes its own small JSON file next to its
//! database: `<db file>.backend.<pid>.json`, holding who it is (`pid`) and
//! where it listens (`port`). Another process on the same profile reads the
//! records to find those backends instead of being refused. Nothing here
//! stops a second app from starting (#2700) — and a second runtime never
//! hides the first one, because each runtime owns its own record.
//!
//! The records also guard startup recovery: a process that finds a live
//! backend must not rewrite that backend's `Busy` / `Queued` sessions.
//!
//! A record counts as live only when its `pid` is another running process
//! and its `port` accepts a loopback connection. The port check covers a
//! crashed backend whose pid the OS has since given to an unrelated
//! process. Records whose pid is definitively dead are swept away.

use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Middle of every record name, between the database file name and the pid.
const RECORD_INFIX: &str = ".backend.";

/// Final extension of every record file.
const RECORD_SUFFIX: &str = ".json";

/// How long the loopback port probe may take.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(300);

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDescriptor {
    /// Process that hosts the backend.
    pub pid: u32,
    /// Loopback port of its Session API.
    pub port: u16,
    /// When that process published the record (Unix milliseconds).
    pub started_at_ms: i64,
}

/// Where the record of one pid's runtime on the given database lives.
///
/// Keyed by the database, not the data dir: the `prod` and `dev` profiles
/// share a data dir but are separate backends with separate databases.
fn record_path_for(db_path: &Path, pid: u32) -> PathBuf {
    let mut name = db_path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(format!("{RECORD_INFIX}{pid}{RECORD_SUFFIX}"));
    db_path.with_file_name(name)
}

/// Set once at startup, before the HTTP server and recovery start.
pub fn init(db_path: &Path) {
    let _ = DB_PATH.set(db_path.to_path_buf());
}

fn read_descriptor(path: &Path) -> Option<BackendDescriptor> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn loopback_port_accepts(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, PORT_PROBE_TIMEOUT).is_ok()
}

/// A pid probe: `Some(true)` alive, `Some(false)` definitively gone, `None`
/// the probe itself failed. A failed probe never sweeps and never matches —
/// recovery must not be blocked forever by a record nobody can verify, and
/// must not delete a record it cannot prove dead.
fn probe_pid(pid: u32) -> Option<bool> {
    crate::utils::process::process_is_alive(pid).ok()
}

/// Scan for live records of other runtimes on this database and return the
/// most recently started one. Records of definitively-dead pids are swept.
fn find_live_backend_with_probes(
    db_path: &Path,
    own_pid: u32,
    pid_probe: impl Fn(u32) -> Option<bool>,
    port_accepts: impl Fn(u16) -> bool,
) -> Option<BackendDescriptor> {
    let prefix = format!(
        "{}{RECORD_INFIX}",
        db_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    );
    let dir = db_path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;
    let mut newest: Option<BackendDescriptor> = None;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(pid_part) = name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(RECORD_SUFFIX))
        else {
            continue;
        };
        let Ok(pid) = pid_part.parse::<u32>() else {
            continue;
        };
        // Our own record is not "another backend", and is never swept here.
        if pid == own_pid {
            continue;
        }
        match pid_probe(pid) {
            Some(false) => {
                // Crash residue from a process the OS has fully released.
                let _ = std::fs::remove_file(entry.path());
                continue;
            }
            Some(true) => {}
            None => continue,
        }
        let Some(descriptor) = read_descriptor(&entry.path()) else {
            continue;
        };
        // A record whose content disagrees with its file name is broken.
        if descriptor.pid != pid || descriptor.port == 0 || !port_accepts(descriptor.port) {
            continue;
        }
        let newer = newest
            .as_ref()
            .is_none_or(|current| descriptor.started_at_ms >= current.started_at_ms);
        if newer {
            newest = Some(descriptor);
        }
    }
    newest
}

/// The backend another live process is hosting for the database given to
/// [`init`], if any. When several are up, the most recently started one.
///
/// `None` before [`init`], when there is no record, or when every record is
/// stale. A frontend that gets `Some` should use that backend's Session API
/// at `127.0.0.1:<port>`.
pub fn find_live_backend() -> Option<BackendDescriptor> {
    let db_path = DB_PATH.get()?;
    find_live_backend_with_probes(
        db_path,
        std::process::id(),
        probe_pid,
        loopback_port_accepts,
    )
}

fn publish_at(db_path: &Path, port: u16) -> std::io::Result<()> {
    let descriptor = BackendDescriptor {
        pid: std::process::id(),
        port,
        started_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    let path = record_path_for(db_path, descriptor.pid);
    let body = serde_json::to_string(&descriptor).map_err(std::io::Error::other)?;
    // Write then rename, so a reader never sees a half-written record. The
    // temp name does not end in the record suffix, so scanners skip it.
    let mut tmp_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Publish this process as a backend for the database given to [`init`].
///
/// Each runtime has its own record file, so publishing never hides or
/// replaces another running backend's record.
pub fn publish(port: u16) -> std::io::Result<()> {
    let db_path = DB_PATH
        .get()
        .ok_or_else(|| std::io::Error::other("backend descriptor not initialized"))?;
    publish_at(db_path, port)
}

fn unpublish_at(db_path: &Path, pid: u32) -> std::io::Result<()> {
    let path = record_path_for(db_path, pid);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Remove this process's own backend record on clean shutdown.
pub fn unpublish() {
    if let Some(db_path) = DB_PATH.get() {
        let _ = unpublish_at(db_path, std::process::id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tempfile::tempdir;

    fn record(pid: u32, port: u16, started_at_ms: i64) -> BackendDescriptor {
        BackendDescriptor {
            pid,
            port,
            started_at_ms,
        }
    }

    fn write_record(db_path: &Path, descriptor: &BackendDescriptor) {
        let path = record_path_for(db_path, descriptor.pid);
        std::fs::write(&path, serde_json::to_string(descriptor).unwrap()).unwrap();
    }

    #[test]
    fn each_runtime_gets_its_own_record_next_to_its_database() {
        let db = Path::new("/data/libragent_v2.dev.db");
        assert_eq!(
            record_path_for(db, 4242),
            Path::new("/data/libragent_v2.dev.db.backend.4242.json")
        );
        // The `prod` and `dev` profiles keep separate record sets.
        assert_ne!(
            record_path_for(db, 1).file_name(),
            record_path_for(Path::new("/data/libragent_v2.db"), 1).file_name()
        );
    }

    #[test]
    fn only_live_other_records_are_found_and_dead_ones_are_swept() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        write_record(&db, &record(100, 3030, 100));
        write_record(&db, &record(200, 3131, 200));
        write_record(&db, &record(300, 3232, 300));

        // pid 100 is dead, pid 200 is alive on an open port, pid 300 is us.
        let found =
            find_live_backend_with_probes(&db, 300, |pid| Some(pid == 200), |port| port == 3131);
        assert_eq!(found, Some(record(200, 3131, 200)));
        // The dead record was swept; the live and our own records remain.
        assert!(!record_path_for(&db, 100).exists());
        assert!(record_path_for(&db, 200).exists());
        assert!(record_path_for(&db, 300).exists());
    }

    #[test]
    fn the_newest_live_record_wins_when_two_backends_are_up() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        write_record(&db, &record(100, 3030, 100));
        write_record(&db, &record(200, 3131, 200));

        let found = find_live_backend_with_probes(
            &db,
            1,
            |pid| Some(pid == 100 || pid == 200),
            |port| port == 3030 || port == 3131,
        );
        assert_eq!(found, Some(record(200, 3131, 200)));
    }

    #[test]
    fn an_unprovable_pid_is_skipped_but_its_record_survives() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        write_record(&db, &record(100, 3030, 100));

        let found = find_live_backend_with_probes(&db, 1, |_pid| None, |_port| true);
        assert_eq!(found, None);
        assert!(record_path_for(&db, 100).exists());
    }

    #[test]
    fn broken_records_are_skipped_without_sweeping() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        // Unreadable body.
        std::fs::write(&record_path_for(&db, 100), "not json").unwrap();
        // Content pid disagrees with the file name.
        std::fs::write(
            &record_path_for(&db, 102),
            serde_json::to_string(&record(77, 3030, 1)).unwrap(),
        )
        .unwrap();

        let found = find_live_backend_with_probes(&db, 1, |_pid| Some(true), |_port| true);
        assert_eq!(found, None);
        assert!(record_path_for(&db, 100).exists());
        assert!(record_path_for(&db, 102).exists());
    }

    #[test]
    fn publish_writes_its_own_record_beside_another_runtime_s() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        let other = record(4242, 3030, 100);
        write_record(&db, &other);

        publish_at(&db, 4040).unwrap();

        let written =
            read_descriptor(&record_path_for(&db, std::process::id())).expect("record written");
        assert_eq!(written.pid, std::process::id());
        assert_eq!(written.port, 4040);
        // The other runtime's record is untouched.
        assert_eq!(read_descriptor(&record_path_for(&db, 4242)), Some(other));
    }

    #[test]
    fn our_own_record_is_not_another_backend() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        publish_at(&db, 3030).unwrap();

        let found = find_live_backend_with_probes(
            &db,
            std::process::id(),
            probe_pid,
            loopback_port_accepts,
        );
        assert_eq!(found, None);
    }

    /// A real process that stays alive until killed — a stand-in for a
    /// second or third LibrAgent runtime.
    fn spawn_stalling_process() -> std::process::Child {
        let mut command = if cfg!(windows) {
            let mut c = std::process::Command::new("ping");
            c.args(["-n", "30", "127.0.0.1"]);
            c
        } else {
            let mut c = std::process::Command::new("sleep");
            c.arg("30");
            c
        };
        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("stalling process")
    }

    fn reap(mut child: std::process::Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    /// The hole the old single-path record had: the second runtime's start
    /// must not hide the first one from a third runtime's recovery guard.
    #[test]
    fn a_third_process_still_finds_the_first_when_a_second_starts() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("libragent_v2.dev.db");

        // Two other live backends: real processes with real listening ports,
        // publishing their own records in startup order.
        let first = spawn_stalling_process();
        let first_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let first_port = first_listener.local_addr().unwrap().port();
        let first_pid = first.id();
        write_record(&db, &record(first_pid, first_port, 100));

        let second = spawn_stalling_process();
        let second_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let second_port = second_listener.local_addr().unwrap().port();
        let second_pid = second.id();
        write_record(&db, &record(second_pid, second_port, 200));

        // This process is the third runtime guarding its startup recovery.
        let find = || {
            find_live_backend_with_probes(&db, std::process::id(), probe_pid, loopback_port_accepts)
        };

        // The second runtime's start did not hide the first one.
        let found = find().expect("both backends discoverable");
        assert_eq!(found.pid, second_pid);

        // When the second dies, the first is still discoverable.
        reap(second);
        drop(second_listener);
        let found = find().expect("first backend still discoverable");
        assert_eq!(found.pid, first_pid);

        // When the first dies too: no live backend, and both records swept.
        reap(first);
        drop(first_listener);
        assert_eq!(find(), None);
        assert!(!record_path_for(&db, first_pid).exists());
        assert!(!record_path_for(&db, second_pid).exists());
    }

    #[test]
    fn unpublish_removes_own_record_only() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("app.db");
        write_record(&db, &record(100, 3030, 100));
        write_record(&db, &record(200, 3131, 200));

        unpublish_at(&db, 100).unwrap();
        assert!(!record_path_for(&db, 100).exists());
        assert!(record_path_for(&db, 200).exists());
        // Idempotent when already removed.
        unpublish_at(&db, 100).unwrap();
    }
}

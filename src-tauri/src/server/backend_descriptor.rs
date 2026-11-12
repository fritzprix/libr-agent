//! Discovery record for the running backend — not a lock.
//!
//! The process that hosts the Session API writes a small JSON file next to
//! its database: who it is (`pid`) and where it listens (`port`). Another
//! process on the same profile reads it to find that backend instead of being
//! refused. Nothing here stops a second app from starting (#2700).
//!
//! The record also guards startup recovery: a process that finds a live
//! backend must not rewrite that backend's `Busy` / `Queued` sessions.
//!
//! A record counts as live only when its `pid` is another running process and
//! its `port` accepts a loopback connection. The port check covers a crashed
//! backend whose pid the OS has since given to an unrelated process.

use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Suffix appended to the database file name.
const DESCRIPTOR_SUFFIX: &str = ".backend.json";

/// How long the loopback port probe may take.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(300);

static DESCRIPTOR_PATH: OnceLock<PathBuf> = OnceLock::new();

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

/// Where the record for the database at `db_path` lives.
///
/// Keyed by the database, not the data dir: the `prod` and `dev` profiles
/// share a data dir but are separate backends with separate databases.
pub fn descriptor_path_for(db_path: &Path) -> PathBuf {
    let mut name = db_path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(DESCRIPTOR_SUFFIX);
    db_path.with_file_name(name)
}

/// Set once at startup, before the HTTP server and recovery start.
pub fn init(db_path: &Path) {
    let _ = DESCRIPTOR_PATH.set(descriptor_path_for(db_path));
}

fn configured_path() -> Option<&'static Path> {
    DESCRIPTOR_PATH.get().map(PathBuf::as_path)
}

fn read_descriptor(path: &Path) -> Option<BackendDescriptor> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn loopback_port_accepts(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, PORT_PROBE_TIMEOUT).is_ok()
}

/// The liveness rule, with the two system probes passed in so it can be tested.
fn is_live_other_backend(
    descriptor: &BackendDescriptor,
    own_pid: u32,
    pid_is_alive: impl Fn(u32) -> bool,
    port_accepts: impl Fn(u16) -> bool,
) -> bool {
    descriptor.pid != own_pid
        && descriptor.port != 0
        && pid_is_alive(descriptor.pid)
        && port_accepts(descriptor.port)
}

fn find_live_backend_at(path: &Path) -> Option<BackendDescriptor> {
    let descriptor = read_descriptor(path)?;
    is_live_other_backend(
        &descriptor,
        std::process::id(),
        // A failed probe is treated as "not alive": the port check below it
        // would have to pass as well, and recovery must not be blocked forever
        // by a record nobody can verify.
        |pid| crate::utils::process::process_is_alive(pid).unwrap_or(false),
        loopback_port_accepts,
    )
    .then_some(descriptor)
}

/// The backend another live process is hosting for this database, if any.
///
/// `None` before [`init`], when there is no record, or when the record is
/// stale. A frontend that gets `Some` should use that backend's Session API
/// at `127.0.0.1:<port>`.
pub fn find_live_backend() -> Option<BackendDescriptor> {
    find_live_backend_at(configured_path()?)
}

fn publish_at(path: &Path, port: u16) -> std::io::Result<bool> {
    if find_live_backend_at(path).is_some() {
        return Ok(false);
    }
    let descriptor = BackendDescriptor {
        pid: std::process::id(),
        port,
        started_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    let body = serde_json::to_string(&descriptor).map_err(std::io::Error::other)?;
    // Write then rename, so a reader never sees a half-written record.
    let tmp = path.with_extension(format!("tmp-{}", descriptor.pid));
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(true)
}

/// Publish this process as the backend, unless another live process already is.
///
/// Returns whether the record now names this process.
pub fn publish(port: u16) -> bool {
    let Some(path) = configured_path() else {
        return false;
    };
    match publish_at(path, port) {
        Ok(published) => published,
        Err(error) => {
            log::warn!(
                "Failed to publish backend descriptor {}: {error}",
                path.display()
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn descriptor(pid: u32, port: u16) -> BackendDescriptor {
        BackendDescriptor {
            pid,
            port,
            started_at_ms: 1,
        }
    }

    #[test]
    fn the_record_sits_next_to_its_database() {
        let path = descriptor_path_for(Path::new("/data/libragent_v2.dev.db"));
        assert_eq!(path, Path::new("/data/libragent_v2.dev.db.backend.json"));
    }

    #[test]
    fn only_another_running_process_with_an_open_port_is_live() {
        let other = descriptor(4242, 3030);
        let alive = |_pid: u32| true;
        let dead = |_pid: u32| false;
        let open = |_port: u16| true;
        let closed = |_port: u16| false;

        assert!(is_live_other_backend(&other, 1, alive, open));
        // Our own record is not "another backend".
        assert!(!is_live_other_backend(&other, 4242, alive, open));
        // Crash residue: the process is gone.
        assert!(!is_live_other_backend(&other, 1, dead, open));
        // The pid was reused by something that does not serve the API.
        assert!(!is_live_other_backend(&other, 1, alive, closed));
        assert!(!is_live_other_backend(&descriptor(4242, 0), 1, alive, open));
    }

    #[test]
    fn publish_writes_a_record_this_process_does_not_treat_as_another_backend() {
        let dir = tempdir().unwrap();
        let path = descriptor_path_for(&dir.path().join("app.db"));

        assert!(publish_at(&path, 3030).unwrap());

        let written = read_descriptor(&path).expect("record written");
        assert_eq!(written.pid, std::process::id());
        assert_eq!(written.port, 3030);
        assert_eq!(find_live_backend_at(&path), None);
    }

    #[cfg(unix)]
    #[test]
    fn publish_does_not_replace_a_live_backend_of_another_process() {
        let dir = tempdir().unwrap();
        let path = descriptor_path_for(&dir.path().join("app.db"));
        // A live "other backend": our parent process, and a port we listen on.
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let other = descriptor(std::os::unix::process::parent_id(), port);
        std::fs::write(&path, serde_json::to_string(&other).unwrap()).unwrap();

        assert_eq!(find_live_backend_at(&path), Some(other.clone()));
        assert!(!publish_at(&path, 4040).unwrap());
        assert_eq!(read_descriptor(&path), Some(other));
    }

    #[test]
    fn a_stale_or_unreadable_record_is_replaced() {
        let dir = tempdir().unwrap();
        let path = descriptor_path_for(&dir.path().join("app.db"));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(find_live_backend_at(&path), None);

        assert!(publish_at(&path, 3030).unwrap());
        assert_eq!(read_descriptor(&path).unwrap().port, 3030);
    }
}

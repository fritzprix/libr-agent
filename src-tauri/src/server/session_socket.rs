//! Per-session unix socket endpoint for managed Docker sessions (#2343).
//!
//! Each managed session gets its own socket under `~/.libragent/sock/`, served by the same
//! router as the main TCP port but with a fixed `origin` (the session that owns the socket).
//! The socket file is bind mounted into the session's container, where a relay exposes it as
//! `127.0.0.1:<port>`. No TCP port is opened on the host. Docker Desktop for macOS can only
//! connect to a socket that is mounted as a single file, not through its parent directory.

use std::path::PathBuf;
use std::sync::OnceLock;

use dashmap::{DashMap, DashSet};
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use tokio::task::JoinHandle;

/// Serves a session's router on its listener until the future is dropped. Registered by
/// `server::init`, which is where the router is built: naming the router type here would make
/// the router and the code that provisions containers depend on each other's types.
#[cfg(unix)]
pub type ServeSession =
    Box<dyn Fn(String, tokio::net::UnixListener) -> BoxFuture<'static, ()> + Send + Sync>;
#[cfg(not(unix))]
pub type ServeSession = Box<dyn Fn(String) -> BoxFuture<'static, ()> + Send + Sync>;

/// Where the socket appears inside the container.
pub const CONTAINER_SOCKET_PATH: &str = "/run/libragent/api.sock";

static SERVE: OnceLock<ServeSession> = OnceLock::new();
static SERVERS: Lazy<DashMap<String, JoinHandle<()>>> = Lazy::new(DashMap::new);
/// Sessions whose container has a relay that reported it is listening. Only
/// `mark_relay_ready` adds to it, so no Docker session is handed a wake URL that leads nowhere.
static RELAYS: Lazy<DashSet<String>> = Lazy::new(DashSet::new);

/// Records how a session's socket is served. Called once by `server::init`.
pub fn register_serve(serve: ServeSession) {
    let _ = SERVE.set(serve);
}

fn is_plain_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// `<LIBRAGENT_DATA_DIR>/sock` for an isolated instance (so it never touches the user's real
/// `~/.libragent`), otherwise `~/.libragent/sock`.
fn socket_dir() -> Result<PathBuf, String> {
    if let Some(data_dir) = std::env::var_os("LIBRAGENT_DATA_DIR") {
        return Ok(PathBuf::from(data_dir).join("sock"));
    }
    let home = dirs::home_dir().ok_or_else(|| "Home directory is not available".to_string())?;
    Ok(home.join(".libragent").join("sock"))
}

/// Host path of a session's socket.
pub fn socket_path(session_id: &str) -> Result<PathBuf, String> {
    if !is_plain_session_id(session_id) {
        return Err(format!(
            "Invalid session id for a socket path: {session_id}"
        ));
    }
    Ok(socket_dir()?.join(format!("{session_id}.sock")))
}

/// Records that the session's relay is listening in its container.
pub fn mark_relay_ready(session_id: &str) {
    RELAYS.insert(session_id.to_string());
}

/// Records that the session's relay is gone (it exited, or the container was removed).
pub fn clear_relay(session_id: &str) {
    RELAYS.remove(session_id);
}

/// True when the session's socket is served and its container relays `127.0.0.1` to it.
pub fn is_reachable_from_container(session_id: &str) -> bool {
    is_serving(session_id) && RELAYS.contains(session_id)
}

/// True while the session's socket is being served.
pub fn is_serving(session_id: &str) -> bool {
    SERVERS
        .get(session_id)
        .is_some_and(|handle| !handle.is_finished())
}

#[cfg(unix)]
pub async fn ensure_session_socket(session_id: &str) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let path = socket_path(session_id)?;
    if is_serving(session_id) && path.exists() {
        return Ok(path);
    }
    let serve = SERVE
        .get()
        .ok_or_else(|| "The HTTP server has not started".to_string())?;

    if let Some((_, stale)) = SERVERS.remove(session_id) {
        stale.abort();
    }
    let dir = path
        .parent()
        .ok_or_else(|| "Socket path has no parent".to_string())?;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("Failed to create '{}': {e}", dir.display()))?;
    tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .await
        .map_err(|e| format!("Failed to restrict '{}': {e}", dir.display()))?;
    match tokio::fs::remove_file(&path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Failed to remove '{}': {e}", path.display())),
    }

    let listener = tokio::net::UnixListener::bind(&path)
        .map_err(|e| format!("Failed to bind '{}': {e}", path.display()))?;
    tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|e| format!("Failed to restrict '{}': {e}", path.display()))?;

    let server = serve(session_id.to_string(), listener);
    let id = session_id.to_string();
    let handle = tokio::spawn(async move {
        server.await;
        log::info!("Session socket for {id} stopped");
    });
    SERVERS.insert(session_id.to_string(), handle);
    log::info!("Session socket for {session_id} at {}", path.display());
    Ok(path)
}

#[cfg(not(unix))]
pub async fn ensure_session_socket(_session_id: &str) -> Result<PathBuf, String> {
    Err("Session sockets are not supported on this platform".to_string())
}

/// Stops serving the session's socket and removes the file.
pub fn remove_session_socket(session_id: &str) {
    RELAYS.remove(session_id);
    if let Some((_, handle)) = SERVERS.remove(session_id) {
        handle.abort();
    }
    if let Ok(path) = socket_path(session_id) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_served_socket_is_not_reachable_without_a_relay() {
        assert!(!is_reachable_from_container("socket-test-no-relay"));
    }

    #[test]
    fn socket_path_rejects_ids_that_could_escape_the_directory() {
        for id in ["", "../x", "a/b", "a\\b", "a.b", "a b"] {
            assert!(socket_path(id).is_err(), "{id:?}");
        }
        assert!(socket_path("abc123-_X").is_ok());
    }

    #[test]
    fn nothing_is_serving_without_a_listener() {
        assert!(!is_serving("socket-test-none"));
    }
}

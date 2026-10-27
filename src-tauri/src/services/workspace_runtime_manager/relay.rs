//! Starts the embedded `session-relay` inside a managed container and keeps track of it.
//!
//! The relay forwards the container's `127.0.0.1:<port>` to the session's unix socket, which is
//! how `$LIBRAGENT_WAKE_URL` reaches the host. It is run with `docker exec`, and the host keeps
//! that `docker exec` process: the relay prints `ready` once it is listening and exits when it
//! cannot own the port, so "ready" is something the relay itself said, not something a probe of
//! the port could confuse with another program listening there. The session is registered as
//! reachable only after that line, and a relay whose process ended is started again.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command as AsyncCommand};
use tokio::sync::Mutex;

use super::docker_cli::{apply_docker_cli_env, run_docker_status};

/// Where the relay binary is placed inside the container.
const CONTAINER_RELAY_PATH: &str = "/tmp/libragent-session-relay";

pub(super) struct RelayEntry {
    pub container_name: String,
    pub child: Mutex<Option<Child>>,
}

/// The running relay state for each session.
static RELAYS: Lazy<DashMap<String, Arc<RelayEntry>>> = Lazy::new(DashMap::new);

/// Checks if the relay binary is actively running inside `container_name`.
pub(super) async fn is_running_in_container(container_name: &str) -> bool {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["top", container_name]);
    match cmd.output().await {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().any(|line| {
                line.contains(CONTAINER_RELAY_PATH) || line.contains("libragent-session-relay")
            })
        }
        _ => false,
    }
}

/// Kills any running relay processes inside `container_name`.
pub(super) async fn terminate_relay_in_container(container_name: &str) {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args([
        "exec",
        container_name,
        "sh",
        "-c",
        "for p in /proc/[0-9]*; do pid=\"${p##*/}\"; if [ \"$pid\" != \"$$\" ] && [ -f \"$p/cmdline\" ] && grep -qa 'libragent-session-relay' \"$p/cmdline\"; then kill \"$pid\" 2>/dev/null; fi; done",
    ]);
    let _ = cmd.status().await;
}

/// Waits for the relay's first line. `Ok` only when it is `ready`; a relay that exits first (port
/// in use, bad arguments) or says anything else is an error.
async fn wait_for_ready(child: &mut Child) -> Result<(), String> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "the relay has no stdout".to_string())?;
    let mut lines = BufReader::new(stdout).lines();
    match lines.next_line().await {
        Ok(Some(line)) if line.trim() == "ready" => Ok(()),
        Ok(Some(line)) => Err(format!("unexpected output from the relay: {line}")),
        Ok(None) | Err(_) => {
            let status = child
                .wait()
                .await
                .map(|status| status.to_string())
                .unwrap_or_else(|error| error.to_string());
            Err(format!("the relay exited before it was ready ({status})"))
        }
    }
}

/// True while the relay's `docker exec` process is still running.
#[allow(dead_code)]
fn is_running(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

/// Writes the relay to a temporary file and copies it into the container.
async fn install(container_name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = temp_relay_path();
    write_executable(&path, bytes)
        .await
        .map_err(|error| format!("could not write the relay to {}: {error}", path.display()))?;
    let target = format!("{container_name}:{CONTAINER_RELAY_PATH}");
    let copied = run_docker_status(["cp", &path.to_string_lossy(), &target]).await;
    let _ = tokio::fs::remove_file(&path).await;
    copied.map_err(|error| format!("docker cp failed: {error}"))
}

fn temp_relay_path() -> PathBuf {
    std::env::temp_dir().join(format!("libragent-session-relay-{}", uuid::Uuid::new_v4()))
}

async fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    tokio::fs::write(path, bytes).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(())
}

/// Makes sure the session's relay is running in `container_name` and registered as reachable.
/// Returns whether it is. `false` means no relay (the build embeds none for this architecture,
/// the socket is not served, the port is in use, or starting it failed): then the session gets no
/// wake URL.
pub(super) async fn ensure_relay(session_id: &str, container_name: &str, arch: &str) -> bool {
    let entry = RELAYS
        .entry(session_id.to_string())
        .or_insert_with(|| {
            Arc::new(RelayEntry {
                container_name: container_name.to_string(),
                child: Mutex::new(None),
            })
        })
        .clone();
    let mut running = entry.child.lock().await;

    // Check if the relay is actively running inside the container (independent of host docker client)
    if is_running_in_container(container_name).await {
        crate::server::session_socket::mark_relay_ready(session_id);
        return true;
    }

    if running.is_some() {
        log::info!("The session relay for {session_id} stopped; starting it again");
        crate::server::session_socket::clear_relay(session_id);
        *running = None;
    }

    if !crate::server::session_socket::is_serving(session_id) {
        return false;
    }
    let Some(bytes) = crate::mcp::builtin::workspace::session_relay::relay_binary(arch) else {
        return false;
    };

    match start(session_id, container_name, bytes).await {
        Ok(child) => {
            *running = Some(child);
            crate::server::session_socket::mark_relay_ready(session_id);
            true
        }
        Err(error) => {
            log::warn!("No session relay for {session_id}: {error}");
            false
        }
    }
}

async fn start(session_id: &str, container_name: &str, bytes: &[u8]) -> Result<Child, String> {
    install(container_name, bytes).await?;
    let port = crate::server::active_http_port();

    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", container_name, CONTAINER_RELAY_PATH])
        .arg(port.to_string())
        .arg(crate::server::session_socket::CONTAINER_SOCKET_PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|error| format!("could not run docker exec for {session_id}: {error}"))?;
    wait_for_ready(&mut child).await?;
    Ok(child)
}

/// Forgets the session's relay: the container is gone or being replaced.
pub(super) async fn forget(session_id: &str, container_name: &str) {
    crate::server::session_socket::clear_relay(session_id);
    if let Some((_, entry)) = RELAYS.remove(session_id) {
        if let Some(mut child) = entry.child.lock().await.take() {
            let _ = child.kill().await;
        }
    }
    terminate_relay_in_container(container_name).await;
}

/// Shuts down all running relays across all containers upon app exit.
pub async fn shutdown_all_relays() {
    let active: Vec<(String, String)> = RELAYS
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().container_name.clone()))
        .collect();
    for (session_id, container_name) in active {
        forget(&session_id, &container_name).await;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(script: &str) -> Child {
        AsyncCommand::new("sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }

    #[tokio::test]
    async fn a_relay_that_says_ready_is_ready_and_keeps_running() {
        let mut child = shell("echo ready; sleep 30");
        assert_eq!(wait_for_ready(&mut child).await, Ok(()));
        assert!(is_running(&mut child));
    }

    #[tokio::test]
    async fn a_relay_that_exits_instead_of_listening_is_not_ready() {
        // The port is already in use: the relay exits with 3 and never prints ready.
        let mut child = shell("exit 3");
        let error = wait_for_ready(&mut child).await.unwrap_err();
        assert!(error.contains("exited before it was ready"), "{error}");
    }

    #[tokio::test]
    async fn another_program_on_the_port_is_not_taken_for_the_relay() {
        let mut child = shell("echo not-the-relay; sleep 30");
        assert!(wait_for_ready(&mut child).await.is_err());
    }

    #[tokio::test]
    async fn a_relay_that_ended_is_noticed() {
        let mut child = shell("echo ready");
        wait_for_ready(&mut child).await.unwrap();
        let _ = child.wait().await;
        assert!(!is_running(&mut child));
    }

    #[tokio::test]
    async fn without_an_embedded_relay_a_session_has_none() {
        // This build embeds no relay for an architecture nobody builds.
        assert!(!ensure_relay("relay-test-none", "container", "riscv64").await);
        assert!(!crate::server::session_socket::is_reachable_from_container(
            "relay-test-none"
        ));
    }

    #[tokio::test]
    async fn forget_cleans_up_relay_and_session_socket() {
        crate::server::session_socket::mark_relay_ready("test-forget-sess");
        assert!(
            crate::server::session_socket::is_reachable_from_container("test-forget-sess") == false
        ); // not serving

        let entry = Arc::new(RelayEntry {
            container_name: "test-container".to_string(),
            child: Mutex::new(Some(shell("sleep 30"))),
        });
        RELAYS.insert("test-forget-sess".to_string(), entry);

        forget("test-forget-sess", "nonexistent-container").await;

        assert!(!RELAYS.contains_key("test-forget-sess"));
    }

    #[tokio::test]
    async fn shutdown_all_relays_cleans_up_all_active_relays() {
        let entry1 = Arc::new(RelayEntry {
            container_name: "c1".to_string(),
            child: Mutex::new(Some(shell("sleep 30"))),
        });
        let entry2 = Arc::new(RelayEntry {
            container_name: "c2".to_string(),
            child: Mutex::new(Some(shell("sleep 30"))),
        });
        RELAYS.insert("s1".to_string(), entry1);
        RELAYS.insert("s2".to_string(), entry2);

        shutdown_all_relays().await;

        assert!(RELAYS.is_empty());
    }
}

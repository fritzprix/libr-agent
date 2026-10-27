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
    /// Serializes starts of this session's relay. Only `ensure_relay` takes it; `forget` never
    /// does, so removing the container is not held up by a relay that is slow to start.
    start_gate: Mutex<()>,
    /// The host's `docker exec` handle, locked only for a moment to read or replace it.
    child: Mutex<Option<Child>>,
}

impl RelayEntry {
    fn new(container_name: &str, child: Option<Child>) -> Arc<Self> {
        Arc::new(Self {
            container_name: container_name.to_string(),
            start_gate: Mutex::new(()),
            child: Mutex::new(child),
        })
    }
}

/// The running relay state for each session.
static RELAYS: Lazy<DashMap<String, Arc<RelayEntry>>> = Lazy::new(DashMap::new);

/// What `docker top` shows about relay processes in a container.
#[derive(Debug, PartialEq, Eq)]
enum RelayProcess {
    /// A relay with exactly this port and socket is running.
    Current,
    /// A relay runs, but for another port or socket (for example the host fell back to another
    /// port after a restart): nothing listens on the port of the current URL.
    Other,
    None,
}

/// Classifies the processes in `docker top` output (`UID PID PPID C STIME TTY TIME CMD`). Only a
/// process whose command starts with the relay's path counts; a process that merely mentions
/// the name in an argument (`pgrep -f libragent-session-relay`) does not.
fn classify_relay_processes(top_output: &str, port: u16, socket: &str) -> RelayProcess {
    let port = port.to_string();
    let mut found = RelayProcess::None;
    for line in top_output.lines().skip(1) {
        let command: Vec<&str> = line.split_whitespace().skip(7).collect();
        if command.first() != Some(&CONTAINER_RELAY_PATH) {
            continue;
        }
        if command.get(1) == Some(&port.as_str()) && command.get(2) == Some(&socket) {
            return RelayProcess::Current;
        }
        found = RelayProcess::Other;
    }
    found
}

/// Looks for the relay inside `container_name` (independent of the host's `docker exec` handle).
/// `None` when the container's processes cannot be listed.
async fn relay_process_in_container(
    container_name: &str,
    port: u16,
    socket: &str,
) -> Option<RelayProcess> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["top", container_name]);
    match cmd.output().await {
        Ok(output) if output.status.success() => Some(classify_relay_processes(
            &String::from_utf8_lossy(&output.stdout),
            port,
            socket,
        )),
        _ => None,
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
/// the socket is not served, the port is in use, the relay could not reach the socket, or
/// starting it failed): then the session gets no wake URL.
pub(super) async fn ensure_relay(session_id: &str, container_name: &str, arch: &str) -> bool {
    let entry = RELAYS
        .entry(session_id.to_string())
        .or_insert_with(|| RelayEntry::new(container_name, None))
        .clone();
    let _start = entry.start_gate.lock().await;

    let port = crate::server::active_http_port();
    let socket = crate::server::session_socket::CONTAINER_SOCKET_PATH;
    match relay_process_in_container(container_name, port, socket).await {
        Some(RelayProcess::Current) => {
            crate::server::session_socket::mark_relay_ready(session_id);
            return true;
        }
        Some(RelayProcess::Other) => {
            log::info!("The session relay for {session_id} serves another port; replacing it");
            terminate_relay_in_container(container_name).await;
        }
        Some(RelayProcess::None) => {}
        None => {
            // Not knowing is not "reachable": no URL until the relay is seen again.
            log::warn!("Cannot list the processes of '{container_name}'; no session relay");
            crate::server::session_socket::clear_relay(session_id);
            return false;
        }
    }

    if entry.child.lock().await.take().is_some() {
        log::info!("The session relay for {session_id} stopped; starting it again");
    }
    crate::server::session_socket::clear_relay(session_id);

    if !crate::server::session_socket::is_serving(session_id) {
        return false;
    }
    let Some(bytes) = crate::mcp::builtin::workspace::session_relay::relay_binary(arch) else {
        return false;
    };

    // The child lock is not held while waiting for `ready`.
    let child = match start(session_id, container_name, bytes, port).await {
        Ok(child) => child,
        Err(error) => {
            log::warn!("No session relay for {session_id}: {error}");
            return false;
        }
    };
    let still_registered = RELAYS
        .get(session_id)
        .is_some_and(|current| Arc::ptr_eq(current.value(), &entry));
    if !still_registered {
        // The container was removed while the relay started.
        drop(child);
        terminate_relay_in_container(container_name).await;
        return false;
    }
    *entry.child.lock().await = Some(child);
    crate::server::session_socket::mark_relay_ready(session_id);
    true
}

async fn start(
    session_id: &str,
    container_name: &str,
    bytes: &[u8],
    port: u16,
) -> Result<Child, String> {
    install(container_name, bytes).await?;

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

/// Forgets the session's relay: the container is gone or being replaced. Does not wait for a
/// relay that is still starting; that start notices the entry is gone and stops its relay.
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
        assert!(matches!(child.try_wait(), Ok(None)));
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
        assert!(!matches!(child.try_wait(), Ok(None)));
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

        let entry = RelayEntry::new("test-container", Some(shell("sleep 30")));
        RELAYS.insert("test-forget-sess".to_string(), entry);

        forget("test-forget-sess", "nonexistent-container").await;

        assert!(!RELAYS.contains_key("test-forget-sess"));
    }

    #[tokio::test]
    async fn shutdown_all_relays_cleans_up_all_active_relays() {
        let entry1 = RelayEntry::new("c1", Some(shell("sleep 30")));
        let entry2 = RelayEntry::new("c2", Some(shell("sleep 30")));
        RELAYS.insert("s1".to_string(), entry1);
        RELAYS.insert("s2".to_string(), entry2);

        shutdown_all_relays().await;

        assert!(RELAYS.is_empty());
    }

    const TOP_HEADER: &str = "UID PID PPID C STIME TTY TIME CMD\n";
    const SOCKET: &str = "/run/libragent/api.sock";

    fn top(commands: &[&str]) -> String {
        let mut out = TOP_HEADER.to_string();
        for (index, command) in commands.iter().enumerate() {
            out.push_str(&format!(
                "root {} 1 0 10:00 ? 00:00:00 {command}\n",
                10 + index
            ));
        }
        out
    }

    #[test]
    fn a_relay_with_the_current_port_and_socket_is_current() {
        let output = top(&[
            "tail -f /dev/null",
            &format!("{CONTAINER_RELAY_PATH} 3030 {SOCKET}"),
        ]);
        assert_eq!(
            classify_relay_processes(&output, 3030, SOCKET),
            RelayProcess::Current
        );
    }

    #[test]
    fn a_relay_for_another_port_is_replaced_not_reused() {
        // The host fell back from 3030 to 3031 after a restart: the old relay is useless.
        let output = top(&[&format!("{CONTAINER_RELAY_PATH} 3030 {SOCKET}")]);
        assert_eq!(
            classify_relay_processes(&output, 3031, SOCKET),
            RelayProcess::Other
        );
    }

    #[test]
    fn a_relay_for_another_socket_is_not_current() {
        let output = top(&[&format!("{CONTAINER_RELAY_PATH} 3030 /run/other.sock")]);
        assert_eq!(
            classify_relay_processes(&output, 3030, SOCKET),
            RelayProcess::Other
        );
    }

    #[test]
    fn unrelated_processes_are_not_a_relay() {
        let output = top(&["tail -f /dev/null", "sleep 1000"]);
        assert_eq!(
            classify_relay_processes(&output, 3030, SOCKET),
            RelayProcess::None
        );
        assert_eq!(
            classify_relay_processes(TOP_HEADER, 3030, SOCKET),
            RelayProcess::None
        );
    }

    #[test]
    fn a_process_that_only_mentions_the_relay_is_not_a_relay() {
        let output = top(&[
            "pgrep -f libragent-session-relay",
            &format!("sh -c {CONTAINER_RELAY_PATH} 3030 {SOCKET}"),
            &format!("grep {CONTAINER_RELAY_PATH} 3030 {SOCKET}"),
        ]);
        assert_eq!(
            classify_relay_processes(&output, 3030, SOCKET),
            RelayProcess::None
        );
    }

    #[test]
    fn a_current_relay_wins_over_a_stale_one() {
        let output = top(&[
            &format!("{CONTAINER_RELAY_PATH} 3030 {SOCKET}"),
            &format!("{CONTAINER_RELAY_PATH} 3031 {SOCKET}"),
        ]);
        assert_eq!(
            classify_relay_processes(&output, 3031, SOCKET),
            RelayProcess::Current
        );
    }

    #[tokio::test]
    async fn forget_does_not_wait_for_a_relay_that_is_still_starting() {
        // `ensure_relay` holds the start gate while the relay starts; `forget` must not need it.
        let entry = RelayEntry::new("c-slow", None);
        let _starting = entry.start_gate.lock().await;
        RELAYS.insert("slow-start-sess".to_string(), entry.clone());

        forget("slow-start-sess", "nonexistent-container").await;

        assert!(!RELAYS.contains_key("slow-start-sess"));
    }
}

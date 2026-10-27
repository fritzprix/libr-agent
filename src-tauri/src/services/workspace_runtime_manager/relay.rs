//! Starts the embedded `session-relay` inside a managed container and keeps track of it.
//!
//! The relay forwards the container's `127.0.0.1:<port>` to the session's unix socket, which is
//! how `$LIBRAGENT_WAKE_URL` reaches the host. It is run with `docker exec`, and the host keeps
//! that `docker exec` process: the relay prints `ready` once it is listening and has reached the
//! socket, and exits otherwise, so "ready" is something the relay itself said, not something a
//! probe of the port could confuse with another program listening there. The session is
//! registered as reachable only after that line.
//!
//! Each start gets an instance id, passed to the relay as an extra argument, so a start that
//! turned out to be stale stops its own relay and nobody else's. A start that fails is
//! remembered for the container, port and socket it was tried with, and is not tried again
//! until one of them changes.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;

use super::docker_cli::{docker_command, run_docker_status};

/// Where the relay binary is placed inside the container.
const CONTAINER_RELAY_PATH: &str = "/tmp/libragent-session-relay";

/// What a registered entry holds. One lock covers "is this entry still the session's" and
/// "which relay it registered", so registering and forgetting cannot interleave.
#[derive(Default)]
struct EntryState {
    /// Set by `forget`. A start that finishes after it must not register anything.
    closed: bool,
    /// The host's `docker exec` handle of the relay this entry started, if any.
    child: Option<Child>,
    /// The instance id of that relay.
    instance: Option<String>,
}

pub(super) struct RelayEntry {
    pub container_name: String,
    /// Serializes starts of this session's relay. Only `ensure_relay` takes it; `forget` never
    /// does, so removing the container is not held up by a relay that is slow to start.
    start_gate: Mutex<()>,
    /// Locked only for a moment, never across an `.await` on Docker.
    state: Mutex<EntryState>,
}

impl RelayEntry {
    fn new(container_name: &str) -> Arc<Self> {
        Arc::new(Self {
            container_name: container_name.to_string(),
            start_gate: Mutex::new(()),
            state: Mutex::new(EntryState::default()),
        })
    }
}

/// The running relay state for each session.
static RELAYS: Lazy<DashMap<String, Arc<RelayEntry>>> = Lazy::new(DashMap::new);

/// What a start was tried with. A failed start is not repeated while all of these stay the same.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Attempt {
    container_name: String,
    port: u16,
    /// The generation of the socket binding; the app re-binding the socket changes it.
    socket: Option<String>,
}

/// The last failed start of each session's relay. Cleared by `forget` (the container is gone or
/// replaced); a new port or socket binding makes it not match.
static FAILED: Lazy<DashMap<String, Attempt>> = Lazy::new(DashMap::new);

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
    let mut cmd = docker_command();
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

/// Kills the relay processes inside `container_name` whose command line contains `pattern`:
/// the relay path for all of them, or an instance id for one start's relay.
async fn terminate_relays_matching(container_name: &str, pattern: &str) {
    let mut cmd = docker_command();
    cmd.args([
        "exec",
        container_name,
        "sh",
        "-c",
        "for p in /proc/[0-9]*; do pid=\"${p##*/}\"; if [ \"$pid\" != \"$$\" ] && [ -f \"$p/cmdline\" ] && grep -qaF -- \"$1\" \"$p/cmdline\" && grep -qa 'libragent-session-relay' \"$p/cmdline\"; then kill \"$pid\" 2>/dev/null; fi; done",
        "terminate-relays",
        pattern,
    ]);
    let _ = cmd.status().await;
}

/// Waits for the relay's first line. `Ok` only when it is `ready`; a relay that exits first (port
/// in use, socket unreachable, bad arguments) or says anything else is an error.
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

/// What became of a start whose relay said `ready`.
#[derive(Debug, PartialEq, Eq)]
enum Registration {
    Registered,
    /// `forget` ran while the relay started: it must be stopped, and only it.
    Stale,
}

/// Registers a started relay on `entry`, or reports that the entry was forgotten meanwhile.
/// The check and the registration (including `mark_relay_ready`) happen under the entry's state
/// lock, which `forget` also takes, so a session is never left marked ready after `forget`.
async fn register_started(
    session_id: &str,
    entry: &RelayEntry,
    child: Option<Child>,
    instance: String,
) -> Registration {
    let mut state = entry.state.lock().await;
    if state.closed {
        return Registration::Stale;
    }
    state.child = child;
    state.instance = Some(instance);
    crate::server::session_socket::mark_relay_ready(session_id);
    Registration::Registered
}

/// Makes sure the session's relay is running in `container_name` and registered as reachable.
/// Returns whether it is. `false` means no relay (the build embeds none for this architecture,
/// the socket is not served, the port is in use, the relay could not reach the socket, or
/// starting it failed): then the session gets no wake URL.
pub(super) async fn ensure_relay(session_id: &str, container_name: &str, arch: &str) -> bool {
    let binary = crate::mcp::builtin::workspace::session_relay::relay_binary(arch);
    ensure_relay_with(session_id, container_name, arch, binary).await
}

/// [`ensure_relay`] with the relay binary passed in (`None`: this build has none for `arch`).
async fn ensure_relay_with(
    session_id: &str,
    container_name: &str,
    arch: &str,
    binary: Option<&[u8]>,
) -> bool {
    let attempt = Attempt {
        container_name: container_name.to_string(),
        port: crate::server::active_http_port(),
        socket: crate::server::session_socket::socket_generation(session_id),
    };
    if FAILED
        .get(session_id)
        .is_some_and(|failed| *failed == attempt)
    {
        // Already failed with this container, port and socket: trying again would fail the same
        // way (and run `docker cp` + `docker exec` on every `runShell`).
        crate::server::session_socket::clear_relay(session_id);
        return false;
    }

    if !crate::server::session_socket::is_serving(session_id) {
        crate::server::session_socket::clear_relay(session_id);
        return false;
    }

    let entry = RELAYS
        .entry(session_id.to_string())
        .or_insert_with(|| RelayEntry::new(container_name))
        .clone();
    let _start = entry.start_gate.lock().await;

    let socket = crate::server::session_socket::CONTAINER_SOCKET_PATH;
    match relay_process_in_container(container_name, attempt.port, socket).await {
        Some(RelayProcess::Current) => {
            // Running in the container, whatever happened to the host's `docker exec` client.
            let state = entry.state.lock().await;
            if state.closed {
                return false;
            }
            crate::server::session_socket::mark_relay_ready(session_id);
            return true;
        }
        Some(RelayProcess::Other) => {
            log::info!("The session relay for {session_id} serves another port; replacing it");
            terminate_relays_matching(container_name, CONTAINER_RELAY_PATH).await;
        }
        Some(RelayProcess::None) => {}
        None => {
            // Not knowing is not "reachable": no URL until the relay is seen again.
            log::warn!("Cannot list the processes of '{container_name}'; no session relay");
            crate::server::session_socket::clear_relay(session_id);
            return false;
        }
    }

    {
        let mut state = entry.state.lock().await;
        if state.child.take().is_some() {
            log::info!("The session relay for {session_id} stopped; starting it again");
        }
        state.instance = None;
    }
    crate::server::session_socket::clear_relay(session_id);

    let Some(bytes) = binary else {
        log::info!("This build embeds no session relay for {arch}; no wake URL for {session_id}");
        FAILED.insert(session_id.to_string(), attempt);
        crate::server::session_socket::clear_relay(session_id);
        return false;
    };

    let instance = uuid::Uuid::new_v4().simple().to_string();
    // No lock is held while waiting for `ready`.
    let child = match start(session_id, container_name, bytes, attempt.port, &instance).await {
        Ok(child) => child,
        Err(error) => {
            log::warn!("No session relay for {session_id}: {error}");
            FAILED.insert(session_id.to_string(), attempt);
            return false;
        }
    };
    match register_started(session_id, &entry, Some(child), instance.clone()).await {
        Registration::Registered => {
            FAILED.remove(session_id);
            true
        }
        Registration::Stale => {
            // The container was forgotten while this relay started. A newer start may already
            // run its own relay in a container of the same name: stop only this one.
            terminate_relays_matching(container_name, &instance).await;
            false
        }
    }
}

async fn start(
    session_id: &str,
    container_name: &str,
    bytes: &[u8],
    port: u16,
    instance: &str,
) -> Result<Child, String> {
    install(container_name, bytes).await?;

    let mut cmd = docker_command();
    cmd.args(["exec", container_name, CONTAINER_RELAY_PATH])
        .arg(port.to_string())
        .arg(crate::server::session_socket::CONTAINER_SOCKET_PATH)
        .arg(instance)
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

/// Forgets the session's relay: the container is gone or being replaced, or the app exits.
/// Does not wait for a relay that is still starting; that start finds the entry closed and stops
/// its own relay. Stops the relay this entry registered, and nothing a newer entry started.
pub(super) async fn forget(session_id: &str, container_name: &str) {
    FAILED.remove(session_id);
    let Some((_, entry)) = RELAYS.remove(session_id) else {
        crate::server::session_socket::clear_relay(session_id);
        return;
    };
    let (child, instance) = {
        let mut state = entry.state.lock().await;
        state.closed = true;
        crate::server::session_socket::clear_relay(session_id);
        (state.child.take(), state.instance.take())
    };
    if let Some(mut child) = child {
        let _ = child.kill().await;
    }
    // A relay this entry only adopted (left by an earlier app run) has no instance id here; the
    // container is being removed, or a later start adopts or replaces it.
    if let Some(instance) = instance {
        terminate_relays_matching(container_name, &instance).await;
    }
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
pub(super) mod tests {
    use super::*;
    use tokio::process::Command as AsyncCommand;

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
        // The port is already in use (3) or the socket is unreachable (5): never `ready`.
        let mut child = shell("exit 5");
        let error = wait_for_ready(&mut child).await.unwrap_err();
        assert!(error.contains("exited before it was ready"), "{error}");
    }

    #[tokio::test]
    async fn another_program_on_the_port_is_not_taken_for_the_relay() {
        let mut child = shell("echo not-the-relay; sleep 30");
        assert!(wait_for_ready(&mut child).await.is_err());
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
            &format!("{CONTAINER_RELAY_PATH} 3030 {SOCKET} 0123abcd"),
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

    /// Serves a session's socket from a temporary data directory, as the app does.
    struct ServedSocket {
        session_id: String,
        dir: PathBuf,
    }

    impl ServedSocket {
        async fn new(session_id: &str) -> Self {
            crate::server::session_socket::register_serve(Box::new(|_, _| {
                Box::pin(std::future::pending::<()>())
            }));
            let dir = std::env::temp_dir().join(format!(
                "rl-{}",
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            ));
            std::env::set_var("LIBRAGENT_DATA_DIR", &dir);
            crate::server::session_socket::ensure_session_socket(session_id)
                .await
                .unwrap();
            std::env::remove_var("LIBRAGENT_DATA_DIR");
            Self {
                session_id: session_id.to_string(),
                dir,
            }
        }
    }

    impl Drop for ServedSocket {
        fn drop(&mut self) {
            std::env::set_var("LIBRAGENT_DATA_DIR", &self.dir);
            crate::server::session_socket::remove_session_socket(&self.session_id);
            std::env::remove_var("LIBRAGENT_DATA_DIR");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// A fake `docker` CLI that logs its arguments and answers `top` from a file, so the
    /// lifecycle runs without a Docker daemon.
    pub(in super::super) struct FakeDocker {
        dir: PathBuf,
    }

    impl FakeDocker {
        pub(in super::super) fn install(top_output: &str) -> Self {
            Self::with_script(top_output, "")
        }

        /// `extra` is shell code run before the default answers (it may `exit`).
        pub(in super::super) fn with_script(top_output: &str, extra: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let dir = std::env::temp_dir().join(format!("fake-docker-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("top.out"), top_output).unwrap();
            let script = dir.join("docker");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\nd=$(dirname \"$0\")\necho \"$*\" >> \"$d/calls.log\"\n{extra}\nif [ \"$1\" = top ]; then cat \"$d/top.out\"; fi\nexit 0\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            super::super::docker_cli::test_docker::use_program(Some(script.into_os_string()));
            Self { dir }
        }

        pub(in super::super) fn calls(&self) -> Vec<String> {
            std::fs::read_to_string(self.dir.join("calls.log"))
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect()
        }
    }

    impl Drop for FakeDocker {
        fn drop(&mut self) {
            super::super::docker_cli::test_docker::use_program(None);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn a_running_relay_is_reused_after_the_docker_client_died_without_a_second_start() {
        // The host's `docker exec` client is gone (the entry holds no child), but the relay
        // still runs in the container with the current port and socket.
        let _socket = ServedSocket::new("client-died").await;
        let port = crate::server::active_http_port();
        let docker = FakeDocker::install(&top(&[&format!(
            "{CONTAINER_RELAY_PATH} {port} {SOCKET} oldinstance"
        )]));
        RELAYS.insert("client-died".to_string(), RelayEntry::new("c-client-died"));

        assert!(ensure_relay("client-died", "c-client-died", "x86_64").await);

        let calls = docker.calls();
        assert_eq!(calls, vec!["top c-client-died".to_string()], "{calls:?}");
        forget("client-died", "c-client-died").await;
    }

    #[tokio::test]
    async fn a_build_without_a_relay_never_touches_docker_again() {
        // No relay embedded for this architecture: the first check looks for a running relay
        // once, then the failure is remembered and later checks run no Docker command at all.
        let _socket = ServedSocket::new("relay-less").await;
        let docker = FakeDocker::install(TOP_HEADER);
        for _ in 0..3 {
            assert!(!ensure_relay("relay-less", "c-relay-less", "riscv64").await);
        }
        assert_eq!(docker.calls(), vec!["top c-relay-less".to_string()]);
        assert_eq!(
            FAILED
                .get("relay-less")
                .map(|failed| failed.container_name.clone()),
            Some("c-relay-less".to_string())
        );
        forget("relay-less", "c-relay-less").await;
    }

    #[tokio::test]
    async fn a_relay_that_cannot_reach_the_socket_is_not_started_again_on_every_check() {
        // macOS with a mount that does not carry the socket: the relay exits 5 without `ready`.
        // The first check copies and runs it once; later checks run no Docker command.
        let _socket = ServedSocket::new("exit-five").await;
        let docker = FakeDocker::with_script(
            TOP_HEADER,
            r#"case "$*" in "exec c-exit-five /tmp/libragent-session-relay "*) exit 5;; esac"#,
        );
        for _ in 0..3 {
            assert!(!ensure_relay_with("exit-five", "c-exit-five", "x86_64", Some(b"relay")).await);
        }
        let calls = docker.calls();
        assert_eq!(calls.len(), 3, "{calls:?}");
        assert_eq!(calls[0], "top c-exit-five");
        assert!(calls[1].starts_with("cp "), "{calls:?}");
        assert!(
            calls[2].starts_with("exec c-exit-five /tmp/libragent-session-relay "),
            "{calls:?}"
        );
        assert!(!crate::server::session_socket::is_reachable_from_container(
            "exit-five"
        ));
        forget("exit-five", "c-exit-five").await;
    }

    #[tokio::test]
    async fn a_failed_start_is_tried_again_only_when_container_port_or_socket_changes() {
        let attempt = Attempt {
            container_name: "c".into(),
            port: 3030,
            socket: Some("gen-1".into()),
        };
        FAILED.insert("remember".to_string(), attempt.clone());
        let same = |other: &Attempt| FAILED.get("remember").is_some_and(|f| *f == *other);

        assert!(same(&attempt));
        assert!(!same(&Attempt {
            container_name: "c2".into(),
            ..attempt.clone()
        }));
        assert!(!same(&Attempt {
            port: 3031,
            ..attempt.clone()
        }));
        assert!(!same(&Attempt {
            socket: Some("gen-2".into()),
            ..attempt.clone()
        }));

        // Removing or replacing the container forgets the failure.
        let _docker = FakeDocker::install(TOP_HEADER);
        forget("remember", "c").await;
        assert!(FAILED.get("remember").is_none());
    }

    #[tokio::test]
    async fn a_start_that_finishes_after_forget_registers_nothing() {
        // Race A: `forget` runs while the relay is starting.
        let entry = RelayEntry::new("c-race-a");
        RELAYS.insert("race-a".to_string(), entry.clone());
        let _docker = FakeDocker::install(TOP_HEADER);

        forget("race-a", "c-race-a").await;
        let outcome =
            register_started("race-a", &entry, Some(shell("sleep 30")), "inst-a".into()).await;

        assert_eq!(outcome, Registration::Stale);
        assert!(!crate::server::session_socket::is_reachable_from_container(
            "race-a"
        ));
        // Nothing was marked ready: the flag set is untouched by the stale start.
        crate::server::session_socket::mark_relay_ready("probe-only");
        crate::server::session_socket::clear_relay("probe-only");
        assert!(entry.state.lock().await.instance.is_none());
    }

    #[tokio::test]
    async fn a_stale_start_stops_only_its_own_relay() {
        // Race B: after `forget`, a newer start registered its relay; the stale start must not
        // stop it.
        let docker = FakeDocker::install(TOP_HEADER);
        let stale = RelayEntry::new("c-race-b");
        RELAYS.insert("race-b".to_string(), stale.clone());
        forget("race-b", "c-race-b").await;

        let newer = RelayEntry::new("c-race-b");
        RELAYS.insert("race-b".to_string(), newer.clone());
        assert_eq!(
            register_started("race-b", &newer, None, "newer-instance".into()).await,
            Registration::Registered
        );

        assert_eq!(
            register_started("race-b", &stale, None, "stale-instance".into()).await,
            Registration::Stale
        );
        terminate_relays_matching("c-race-b", "stale-instance").await;

        let calls = docker.calls();
        assert!(
            calls.iter().any(|call| call.ends_with("stale-instance")),
            "{calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call.contains("newer-instance")),
            "{calls:?}"
        );
        assert_eq!(
            newer.state.lock().await.instance.as_deref(),
            Some("newer-instance")
        );
        forget("race-b", "c-race-b").await;
    }

    #[tokio::test]
    async fn forget_stops_the_relay_it_registered_by_instance() {
        let docker = FakeDocker::install(TOP_HEADER);
        let entry = RelayEntry::new("c-forget");
        RELAYS.insert("forget-own".to_string(), entry.clone());
        register_started(
            "forget-own",
            &entry,
            Some(shell("sleep 30")),
            "own-inst".into(),
        )
        .await;

        forget("forget-own", "c-forget").await;

        assert!(!RELAYS.contains_key("forget-own"));
        assert!(entry.state.lock().await.closed);
        let calls = docker.calls();
        assert!(
            calls
                .iter()
                .any(|call| call.starts_with("exec c-forget") && call.ends_with("own-inst")),
            "{calls:?}"
        );
    }

    #[tokio::test]
    async fn forget_does_not_wait_for_a_relay_that_is_still_starting() {
        // `ensure_relay` holds the start gate while the relay starts; `forget` must not need it.
        let _docker = FakeDocker::install(TOP_HEADER);
        let entry = RelayEntry::new("c-slow");
        let _starting = entry.start_gate.lock().await;
        RELAYS.insert("slow-start-sess".to_string(), entry.clone());

        forget("slow-start-sess", "c-slow").await;

        assert!(!RELAYS.contains_key("slow-start-sess"));
    }

    #[tokio::test]
    async fn shutdown_all_relays_cleans_up_all_active_relays() {
        let _docker = FakeDocker::install(TOP_HEADER);
        RELAYS.insert("s1".to_string(), RelayEntry::new("c1"));
        RELAYS.insert("s2".to_string(), RelayEntry::new("c2"));

        shutdown_all_relays().await;

        assert!(!RELAYS.contains_key("s1") && !RELAYS.contains_key("s2"));
    }
}

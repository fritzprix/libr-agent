mod container;
mod container_ops;
mod docker_cli;
mod error;
mod exec_command;
mod exec_env;
mod provisioning;
mod root_exec;
mod session_config;
mod shell_exec;

use std::sync::Arc;

use crate::repositories::SessionMetadata;
use crate::session_isolation::ShellType;
use tokio::sync::Notify;

use docker_cli::run_docker_status;
use error::{as_docker_unavailable, RuntimeResult};
use provisioning::{
    prepare_docker_runtime, wait_for_docker_provisioning, DOCKER_ARCH_CACHE,
    DOCKER_PROVISIONING_IN_FLIGHT, DOCKER_PROVISIONING_WAITERS, DOCKER_SHELL_CACHE,
};

pub use docker_cli::{docker_bridge_gateway, normalize_docker_arch};
pub use error::{
    WorkspaceRuntimeError, AGENT_ERROR_DOCKER_NOT_AVAILABLE, AGENT_ERROR_DOCKER_NOT_INSTALLED,
};
pub use exec_command::{DockerMcpCommand, RootExecBlocker};
pub use root_exec::{as_root_requested, stopped_processes_note, RootExecLease};
pub use session_config::build_docker_volume_args;
pub(crate) use session_config::{refuse_home_workspace, session_docker_workdir};

pub type DockerStepReporter = Arc<dyn Fn(&str) + Send + Sync>;

pub struct WorkspaceRuntimeManager;

impl WorkspaceRuntimeManager {
    pub async fn healthcheck() -> RuntimeResult<()> {
        run_docker_status(["--version"])
            .await
            .map_err(as_docker_unavailable)?;
        run_docker_status(["info"])
            .await
            .map_err(as_docker_unavailable)?;
        Ok(())
    }

    /// Ensures the Docker runtime is healthy and the session container exists.
    /// Acquires a per-session lock to serialize container creation.
    pub async fn ensure_runtime(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, false, false, None)
            .await
            .map(|_| ())
    }

    /// Returns the cached Docker shell for a session, if runtime has been prepared.
    pub fn cached_docker_shell(session_id: &str) -> Option<ShellType> {
        DOCKER_SHELL_CACHE.get(session_id).map(|entry| *entry)
    }

    /// Returns the cached Docker container architecture for a session, if known.
    pub fn cached_docker_arch(session_id: &str) -> Option<String> {
        DOCKER_ARCH_CACHE.get(session_id).map(|entry| entry.clone())
    }

    /// Cache-aware variant used by high-frequency shell execution paths.
    pub async fn ensure_runtime_cached(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, true, true, None)
            .await
            .map(|_| ())
    }

    pub fn try_mark_provisioning_in_flight(session_id: &str) -> bool {
        if DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id) {
            return false;
        }
        DOCKER_PROVISIONING_IN_FLIGHT.insert(session_id.to_string(), ());
        DOCKER_PROVISIONING_WAITERS
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Notify::new()));
        true
    }

    pub fn clear_provisioning_in_flight(session_id: &str) {
        DOCKER_PROVISIONING_IN_FLIGHT.remove(session_id);
        if let Some((_, notify)) = DOCKER_PROVISIONING_WAITERS.remove(session_id) {
            notify.notify_waiters();
        }
    }

    pub fn is_provisioning_in_flight(session_id: &str) -> bool {
        DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id)
    }

    pub async fn provision_runtime_with_steps(
        session: &SessionMetadata,
        reporter: Option<DockerStepReporter>,
    ) -> RuntimeResult<()> {
        prepare_docker_runtime(session, false, false, reporter)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod exec_env_tests {
    use std::collections::BTreeMap;

    use tokio::process::Command as AsyncCommand;

    use crate::models::workspace_isolation::WorkspaceIsolationMode;
    use crate::repositories::SessionMetadata;
    use crate::session_isolation::ShellType;

    use super::exec_command::{build_docker_exec_command, build_docker_mcp_command};
    use super::exec_env::{apply_exec_env, apply_secret_exec_env};
    use super::{RootExecBlocker, WorkspaceRuntimeManager};

    fn args_and_env(env: Vec<(&str, &str)>) -> (Vec<String>, Vec<(String, Option<String>)>) {
        let mut cmd = AsyncCommand::new("docker");
        apply_exec_env(
            &mut cmd,
            env.into_iter().map(|(k, v)| (k.to_string(), v.to_string())),
        )
        .unwrap();
        let std_cmd = cmd.as_std();
        let args = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let envs = std_cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        (args, envs)
    }

    #[test]
    fn wake_url_value_stays_out_of_the_docker_arguments() {
        let url = "http://host.docker.internal:4000/api/sessions/s/wake/secrettoken";
        let (args, envs) = args_and_env(vec![("LIBRAGENT_WAKE_URL", url), ("PLAIN", "1")]);
        assert!(args.iter().all(|arg| !arg.contains("secrettoken")));
        assert!(args.contains(&"LIBRAGENT_WAKE_URL".to_string()));
        assert!(args.contains(&"PLAIN=1".to_string()));
        assert!(envs.contains(&("LIBRAGENT_WAKE_URL".to_string(), Some(url.to_string()))));
    }

    #[test]
    fn secret_values_stay_out_of_the_docker_arguments() {
        let mut cmd = AsyncCommand::new("docker");
        let secrets = BTreeMap::from([("API_TOKEN".to_string(), "s3cretvalue".to_string())]);
        apply_secret_exec_env(&mut cmd, &secrets).unwrap();
        let std_cmd = cmd.as_std();
        let args: Vec<String> = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["-e".to_string(), "API_TOKEN".to_string()]);
        let value = std_cmd
            .get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new("API_TOKEN"))
            .and_then(|(_, v)| v);
        assert_eq!(value, Some(std::ffi::OsStr::new("s3cretvalue")));
    }

    fn docker_session(env: Vec<(&str, &str)>) -> SessionMetadata {
        SessionMetadata {
            id: "s1".to_string(),
            name: None,
            status: crate::repositories::SessionStatus::Idle,
            model: "m".to_string(),
            provider: "p".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: WorkspaceIsolationMode::Docker,
            docker_config: Some(crate::models::workspace_isolation::DockerWorkspaceConfig {
                image: Some("python:3.12-slim".to_string()),
                attach_container: None,
                workdir: Some("/work".to_string()),
                manage_lifecycle: None,
                env: env
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                port_bindings: Vec::new(),
            }),
            docker_container_name: Some("libragent-session-s1".to_string()),
            docker_host_workspace_path: Some("/tmp/s1".to_string()),
            status_reason: None,
        }
    }

    fn command_parts(cmd: &AsyncCommand) -> (Vec<String>, Vec<(String, Option<String>)>) {
        let std_cmd = cmd.as_std();
        let args = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let envs = std_cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        (args, envs)
    }

    #[test]
    fn mcp_command_execs_argv_in_the_session_container() {
        let session = docker_session(vec![]);
        let built = build_docker_mcp_command(
            &session,
            "npx",
            &["-y".to_string(), "@scope/server".to_string()],
            &std::collections::HashMap::new(),
        )
        .unwrap();
        assert_eq!(built.container, "libragent-session-s1");
        assert!(built.env_file.is_none());
        let (args, _) = command_parts(&built.command);
        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-w",
                "/work",
                "libragent-session-s1",
                "npx",
                "-y",
                "@scope/server"
            ]
        );
    }

    #[test]
    fn mcp_command_passes_env_values_in_a_private_file() {
        let session = docker_session(vec![("SESSION_VAR", "from-session")]);
        let env = std::collections::HashMap::from([
            ("API_TOKEN".to_string(), "secret-value".to_string()),
            ("SESSION_VAR".to_string(), "from-server".to_string()),
            (
                "LIBRAGENT_SESSION_API_KEY".to_string(),
                "libr_key".to_string(),
            ),
        ]);
        let built = build_docker_mcp_command(&session, "node", &[], &env).unwrap();
        let (args, envs) = command_parts(&built.command);
        let env_file = built.env_file.as_ref().expect("env file");
        let path = env_file.to_string_lossy().into_owned();

        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-w",
                "/work",
                "--env-file",
                path.as_str(),
                "libragent-session-s1",
                "node"
            ]
        );
        assert_eq!(
            std::fs::read_to_string(env_file).unwrap(),
            "API_TOKEN=secret-value\nSESSION_VAR=from-server\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(env_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(envs
            .iter()
            .all(|(_, value)| value.as_deref() != Some("secret-value")));

        let file_path = env_file.to_path_buf();
        drop(built);
        assert!(!file_path.exists(), "env file is removed with the command");
    }

    #[test]
    fn server_env_does_not_change_how_the_docker_cli_connects() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::from([
            ("HOME".to_string(), "/container/home".to_string()),
            (
                "DOCKER_HOST".to_string(),
                "tcp://elsewhere:2375".to_string(),
            ),
            ("DOCKER_CONTEXT".to_string(), "other".to_string()),
            (
                "DOCKER_CONFIG".to_string(),
                "/container/.docker".to_string(),
            ),
            ("PATH".to_string(), "/opt/node/bin".to_string()),
        ]);
        let built = build_docker_mcp_command(&session, "node", &[], &env).unwrap();
        let (_, envs) = command_parts(&built.command);
        for (key, value) in &env {
            assert!(
                !envs.contains(&(key.clone(), Some(value.clone()))),
                "server value of {key} reached the docker CLI env"
            );
        }
        let file = std::fs::read_to_string(built.env_file.as_ref().unwrap()).unwrap();
        assert!(file.contains("DOCKER_HOST=tcp://elsewhere:2375\n"));
        assert!(file.contains("HOME=/container/home\n"));
    }

    #[test]
    fn env_value_with_a_line_break_is_rejected() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::from([("A".to_string(), "x\nB=y".to_string())]);
        assert!(build_docker_mcp_command(&session, "node", &[], &env).is_err());
    }

    #[test]
    fn mcp_command_does_not_forward_host_env_into_the_container() {
        let session = docker_session(vec![]);
        let built =
            build_docker_mcp_command(&session, "node", &[], &std::collections::HashMap::new())
                .unwrap();
        let (args, envs) = command_parts(&built.command);
        assert!(!args.iter().any(|arg| arg == "-e" || arg == "--env-file"));
        // The docker CLI itself still gets the isolated host env (PATH, HOME, …).
        assert!(envs.iter().any(|(key, _)| key == "PATH"));
    }

    #[test]
    fn shell_command_runs_as_root_only_when_asked() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::new();

        let root = build_docker_exec_command(
            &session,
            ShellType::Bash,
            "apt-get update",
            &env,
            &BTreeMap::new(),
            None,
            true,
        )
        .unwrap();
        let (args, _) = command_parts(&root);
        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-u",
                "0",
                "-w",
                "/work",
                "libragent-session-s1",
                "bash",
                "-lc",
                "apt-get update"
            ]
        );

        let user = build_docker_exec_command(
            &session,
            ShellType::Bash,
            "id",
            &env,
            &BTreeMap::new(),
            None,
            false,
        )
        .unwrap();
        let (args, _) = command_parts(&user);
        assert!(!args.iter().any(|arg| arg == "-u"));
    }

    #[test]
    fn shell_command_secrets_stay_out_of_the_docker_exec_arguments() {
        let session = docker_session(vec![]);
        let secrets = BTreeMap::from([("API_TOKEN".to_string(), "tok-s3cr3t".to_string())]);
        let env = std::collections::HashMap::from([(
            crate::server::wake::WAKE_URL_ENV.to_string(),
            "http://host.docker.internal:1/wake".to_string(),
        )]);

        let cmd = build_docker_exec_command(
            &session,
            ShellType::Bash,
            "echo $API_TOKEN",
            &env,
            &secrets,
            None,
            false,
        )
        .unwrap();
        let (args, envs) = command_parts(&cmd);
        assert!(args.iter().all(|arg| !arg.contains("tok-s3cr3t")));
        let secret_flag = args.iter().position(|arg| arg == "API_TOKEN").unwrap();
        let wake_flag = args
            .iter()
            .position(|arg| arg == crate::server::wake::WAKE_URL_ENV)
            .unwrap();
        assert!(secret_flag < wake_flag, "app variables come after secrets");
        assert!(envs
            .iter()
            .any(|(key, value)| key == "API_TOKEN" && value.as_deref() == Some("tok-s3cr3t")));
    }

    #[test]
    fn root_is_available_only_in_a_managed_session_container() {
        let managed = docker_session(vec![]);
        assert_eq!(WorkspaceRuntimeManager::root_exec_blocker(&managed), None);

        let mut attached = docker_session(vec![]);
        if let Some(config) = attached.docker_config.as_mut() {
            config.attach_container = Some("dev-box".to_string());
        }
        assert_eq!(
            WorkspaceRuntimeManager::root_exec_blocker(&attached),
            Some(RootExecBlocker::AttachedContainer)
        );

        let mut host = docker_session(vec![]);
        host.workspace_isolation = WorkspaceIsolationMode::Host;
        assert_eq!(
            WorkspaceRuntimeManager::root_exec_blocker(&host),
            Some(RootExecBlocker::HostSession)
        );
    }

    #[test]
    fn session_api_key_variables_are_never_passed() {
        let (args, envs) = args_and_env(vec![
            ("LIBRAGENT_SESSION_API_KEY", "libr_secret"),
            ("LIBRAGENT_SESSION_API_KEY_FILE", "/tmp/key"),
        ]);
        assert!(args.is_empty());
        assert!(envs.is_empty());
    }
}

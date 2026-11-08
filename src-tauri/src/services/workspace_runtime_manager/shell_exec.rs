//! Shell and MCP command entry points on the manager: docker exec command
//! creation, root-exec policy, and the persistent shell spawn.

use std::collections::BTreeMap;
use std::process::Stdio;

use tokio::process::Command as AsyncCommand;

use crate::models::workspace_isolation::{
    validate_env_key, validate_env_value, WorkspaceIsolationMode,
};
use crate::repositories::SessionMetadata;
use crate::session_isolation::{ShellDialect, ShellType, SpawnedShell};

use super::docker_cli::apply_docker_cli_env;
use super::error::RuntimeResult;
use super::exec_command::{
    build_docker_exec_command, build_docker_mcp_command, DockerMcpCommand, RootExecBlocker,
};
use super::exec_env::apply_secret_exec_env;
use super::provisioning::{prepare_docker_runtime, wait_for_docker_provisioning};
use super::root_exec;
use super::session_config::{
    docker_container_name, docker_host_workspace_path, path_mapper_for_session,
};
use super::{session_docker_workdir, WorkspaceRuntimeError, WorkspaceRuntimeManager};

impl WorkspaceRuntimeManager {
    pub async fn create_docker_exec_command(
        session: &SessionMetadata,
        command: &str,
        env_vars: &std::collections::HashMap<String, String>,
        secrets: &BTreeMap<String, String>,
        cwd_override: Option<&str>,
        run_as_root: bool,
    ) -> RuntimeResult<AsyncCommand> {
        if run_as_root {
            if let Some(blocker) = Self::root_exec_blocker(session) {
                return Err(WorkspaceRuntimeError::InvalidConfig(
                    blocker.message().to_string(),
                ));
            }
        }
        let shell = prepare_docker_runtime(session, true, true, None).await?;
        if run_as_root {
            root_exec::verify_root_exec_ready(session).await?;
        }
        build_docker_exec_command(
            session,
            shell,
            command,
            env_vars,
            secrets,
            cwd_override,
            run_as_root,
        )
    }

    /// Why a shell command of this session cannot run as root, or `None` when it can.
    /// Only a container that LibrAgent created and manages allows root.
    pub fn root_exec_blocker(session: &SessionMetadata) -> Option<RootExecBlocker> {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Some(RootExecBlocker::HostSession);
        }
        if session
            .docker_config
            .as_ref()
            .is_some_and(|config| config.is_attach())
        {
            return Some(RootExecBlocker::AttachedContainer);
        }
        None
    }

    /// Builds `docker exec -i -w <workdir> <container> <command> <args...>` for a stdio MCP
    /// server of a Docker session. Argv goes unchanged (no shell). Only the session Docker env
    /// and the server env reach the container, never the host env.
    pub async fn create_docker_mcp_command(
        session: &SessionMetadata,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> RuntimeResult<DockerMcpCommand> {
        Self::prepare_mcp_container(session).await?;
        build_docker_mcp_command(session, command, args, env)
    }

    /// Waits for provisioning and makes sure the session container runs (may pull an image).
    pub async fn prepare_mcp_container(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, true, true, None)
            .await
            .map(|_| ())
    }

    /// `secrets` are passed as `-e KEY` with the value in the docker CLI env, so no value
    /// appears in the host process list.
    pub async fn spawn_docker_persistent_shell(
        session: &SessionMetadata,
        secrets: &BTreeMap<String, String>,
    ) -> RuntimeResult<SpawnedShell> {
        let shell = prepare_docker_runtime(session, false, false, None).await?;

        let container_name = docker_container_name(session)?;
        let host_workspace = docker_host_workspace_path(session)?;
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", &workdir]);

        apply_secret_exec_env(&mut cmd, secrets)?;

        if let Some(config) = &session.docker_config {
            for (key, value) in &config.env {
                validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                cmd.arg("-e").arg(format!("{key}={value}"));
            }
        }

        cmd.arg(container_name);
        match shell {
            ShellType::Bash => cmd.args(["bash", "--norc", "--noprofile"]),
            ShellType::Sh => cmd.arg("sh"),
            ShellType::PowerShell => unreachable!("Docker Unix shell cannot be PowerShell"),
        };
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            WorkspaceRuntimeError::Io(format!("Failed to spawn Docker persistent shell: {e}"))
        })?;

        Ok(SpawnedShell {
            child,
            initial_cwd: workdir,
            path_mapper: path_mapper_for_session(session, host_workspace),
            shell_type: shell,
            shell_dialect: match shell {
                ShellType::Bash => ShellDialect::Bash,
                ShellType::Sh => ShellDialect::Sh,
                ShellType::PowerShell => unreachable!("Docker Unix shell cannot be PowerShell"),
            },
        })
    }
}

//! `docker exec` command builders: the stdio MCP command, the shell command,
//! and the root-exec availability policy.

use std::collections::BTreeMap;

use tokio::process::Command as AsyncCommand;

use crate::models::workspace_isolation::{validate_env_key, validate_env_value};
use crate::repositories::SessionMetadata;
use crate::session_isolation::ShellType;

use super::docker_cli::{apply_docker_cli_env, docker_program};
use super::error::RuntimeResult;
use super::exec_env::{apply_exec_env, apply_secret_exec_env, write_exec_env_file};
use super::session_config::{docker_container_name, session_docker_workdir};
use super::WorkspaceRuntimeError;

/// `docker exec` client for a stdio MCP server in a session container.
pub struct DockerMcpCommand {
    pub command: AsyncCommand,
    pub container: String,
    /// The CLI reads this file when the process starts; keep it for the process lifetime.
    pub env_file: Option<tempfile::TempPath>,
}

pub(super) fn build_docker_mcp_command(
    session: &SessionMetadata,
    command: &str,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
) -> RuntimeResult<DockerMcpCommand> {
    let container = docker_container_name(session)?;
    let mut cmd = AsyncCommand::new(docker_program());
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", "-i", "-w", &session_docker_workdir(session)]);

    let mut merged_env = session
        .docker_config
        .as_ref()
        .map(|config| config.env.clone())
        .unwrap_or_default();
    merged_env.extend(env.iter().map(|(k, v)| (k.clone(), v.clone())));
    let env_file = write_exec_env_file(merged_env)?;
    if let Some(path) = &env_file {
        cmd.arg("--env-file").arg(path.as_os_str());
    }

    cmd.arg(&container).arg(command).args(args);
    Ok(DockerMcpCommand {
        command: cmd,
        container,
        env_file,
    })
}

/// Reason that root execution is not available for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootExecBlocker {
    /// The session runs commands on the host.
    HostSession,
    /// The session uses a container that LibrAgent did not create.
    AttachedContainer,
}

impl RootExecBlocker {
    pub fn message(self) -> &'static str {
        match self {
            Self::HostSession => {
                "asRoot is only available in a Docker session that uses a LibrAgent-managed container"
            }
            Self::AttachedContainer => {
                "asRoot is not available for an attached container: LibrAgent does not manage it"
            }
        }
    }
}

/// Builds `docker exec -i [-u 0] -w <workdir> [env] <container> <shell> -lc <command>`.
pub(super) fn build_docker_exec_command(
    session: &SessionMetadata,
    shell: ShellType,
    command: &str,
    env_vars: &std::collections::HashMap<String, String>,
    secrets: &BTreeMap<String, String>,
    cwd_override: Option<&str>,
    run_as_root: bool,
) -> RuntimeResult<AsyncCommand> {
    let container_name = docker_container_name(session)?;
    let workdir = cwd_override
        .map(str::to_string)
        .unwrap_or_else(|| session_docker_workdir(session));
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", "-i"]);
    if run_as_root {
        // The container itself runs as the host user; only this exec gets uid 0.
        cmd.args(["-u", "0"]);
    }
    cmd.args(["-w", &workdir]);

    let mut merged_env = session
        .docker_config
        .as_ref()
        .map(|config| config.env.clone())
        .unwrap_or_default();
    for (key, value) in env_vars {
        validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        merged_env.insert(key.clone(), value.clone());
    }

    // Later `-e` flags win, so secrets go first.
    apply_secret_exec_env(&mut cmd, secrets)?;
    apply_exec_env(&mut cmd, merged_env)?;

    cmd.arg(container_name);
    // runShell intentionally executes shell syntax; this mirrors the host shell path.
    cmd.args([shell.command(), "-lc", command]);
    Ok(cmd)
}

//! `docker exec` commands for session containers: env and secret passing, the private env
//! file for stdio MCP servers, and the Session API key filter.

use std::collections::BTreeMap;

use crate::models::workspace_isolation::{validate_env_key, validate_env_value};
use crate::repositories::SessionMetadata;
use crate::session_isolation::ShellType;
use tokio::process::Command as AsyncCommand;

use super::docker_cli::{apply_docker_cli_env, docker_program};
use super::error::RuntimeResult;
use super::session_config::{docker_container_name, session_docker_workdir};
use super::WorkspaceRuntimeError;

/// Passes `env` to `docker exec`. The wake URL is given as `-e KEY` with its value in the
/// docker CLI's own environment, so its token never appears in the host process list.
pub(super) fn apply_exec_env(
    cmd: &mut AsyncCommand,
    env: impl IntoIterator<Item = (String, String)>,
) -> RuntimeResult<()> {
    for (key, value) in env {
        validate_env_key(&key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(&key, &value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        if is_session_api_key_var(&key) {
            continue;
        }
        if key == crate::server::wake::WAKE_URL_ENV {
            cmd.arg("-e").arg(&key);
            cmd.env(&key, &value);
        } else {
            cmd.arg("-e").arg(format!("{key}={value}"));
        }
    }
    Ok(())
}

/// Passes session secrets to `docker exec` as `-e KEY`; each value goes only into the
/// docker CLI's own environment, never into the host process list.
pub(super) fn apply_secret_exec_env(
    cmd: &mut AsyncCommand,
    secrets: &BTreeMap<String, String>,
) -> RuntimeResult<()> {
    for (key, value) in secrets {
        validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        cmd.arg("-e").arg(key);
        cmd.env(key, value);
    }
    Ok(())
}

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

/// Writes the server env to a private (0600) temp file for `docker exec --env-file`. Values
/// stay out of the host process list, and out of the docker CLI's own environment: there,
/// `HOME`, `DOCKER_HOST`, `DOCKER_CONTEXT` or `DOCKER_CONFIG` would change which daemon the
/// CLI uses.
pub(super) fn write_exec_env_file(
    env: std::collections::HashMap<String, String>,
) -> RuntimeResult<Option<tempfile::TempPath>> {
    use std::io::Write;

    let mut entries = Vec::with_capacity(env.len());
    for (key, value) in env {
        validate_env_key(&key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(&key, &value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        if value.contains(['\n', '\r']) {
            return Err(WorkspaceRuntimeError::InvalidConfig(format!(
                "Env value of '{key}' contains a line break; stdio MCP servers in a Docker \
                 session cannot receive it"
            )));
        }
        if !is_session_api_key_var(&key) {
            entries.push((key, value));
        }
    }
    if entries.is_empty() {
        return Ok(None);
    }
    entries.sort();

    let io_error =
        |e: std::io::Error| WorkspaceRuntimeError::Io(format!("Failed to write MCP env file: {e}"));
    let mut file = tempfile::Builder::new()
        .prefix("libragent-mcp-env-")
        .tempfile()
        .map_err(io_error)?;
    for (key, value) in &entries {
        writeln!(file, "{key}={value}").map_err(io_error)?;
    }
    file.flush().map_err(io_error)?;
    Ok(Some(file.into_temp_path()))
}

/// The Session API key must never reach a container, whatever the caller passes.
pub(super) fn is_session_api_key_var(key: &str) -> bool {
    key == crate::server::api_key::ENV_VAR || key == crate::server::api_key::ENV_FILE_VAR
}

//! Env passing to `docker exec`: validation, secret/wake-URL handling, and the
//! private env file used by stdio MCP servers.

use std::collections::BTreeMap;

use tokio::process::Command as AsyncCommand;

use crate::models::workspace_isolation::{validate_env_key, validate_env_value};

use super::error::RuntimeResult;
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

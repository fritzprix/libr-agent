//! Shared Docker container ↔ host path mapping for file-oriented tools
//! (workspace, media, …).

use std::path::{Path, PathBuf};

use super::path_mapper::PathMappingLayer;
use crate::models::workspace_isolation::{WorkspaceIsolationMode, DEFAULT_DOCKER_WORKDIR};
use crate::repositories::SessionRepository;

/// Stable substring in the outside-workdir Docker mapping error.
pub const OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER: &str = "workspace file tools only map";

/// Stable substring in the error for LibrAgent host data-dir paths, which exist
/// on the host only and are invisible to both file tools and the container shell.
pub const HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER: &str = "is a LibrAgent host path";

fn is_under_host_data_dir(path: &Path, data_dir: &Path) -> bool {
    path.starts_with(data_dir)
}

fn host_data_dir_not_in_container_error(path_str: &str, workdir: &str) -> String {
    format!(
        "'{path_str}' {HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER} (data/skills directory). It is not mounted in the container, so workspace__runShell cannot see it either. Read skill files through the @system-skills/… or @user-skills/… alias; to run a script inside the container, first copy it into {workdir} with workspace__importFiles (srcAbsPath = this host path)."
    )
}

/// Map an absolute container path to the host workspace for Docker sessions.
///
/// - Non-absolute paths → `Ok(None)` (caller joins the workspace root).
/// - Non-Docker / missing repo → `Ok(None)`.
/// - Absolute path under the session workdir → `Ok(Some(host_path))`.
/// - Absolute path outside the workdir → `Err` with
///   [`OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER`] (file tools cannot map it).
pub async fn map_docker_container_file_tool_path(
    session_id: &str,
    path_str: &str,
) -> Result<Option<PathBuf>, String> {
    if !path_str.starts_with('/') {
        return Ok(None);
    }

    let Some(session_repo) = crate::state::try_get_session_repository() else {
        return Ok(None);
    };
    let Some(session) = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
    else {
        return Ok(None);
    };

    if session.workspace_isolation != WorkspaceIsolationMode::Docker {
        return Ok(None);
    }

    let host_workspace = session
        .docker_host_workspace_path
        .as_ref()
        .ok_or_else(|| format!("Missing Docker host workspace path for session {session_id}"))?;
    let workdir = session
        .docker_config
        .as_ref()
        .map(|config| config.workdir().to_string())
        .unwrap_or_else(|| DEFAULT_DOCKER_WORKDIR.to_string());
    let mapper = PathMappingLayer::with_container_root(PathBuf::from(host_workspace), &workdir);
    let Some(host_path) = mapper.container_to_host(path_str) else {
        let data_dir = crate::profile::data_dir(crate::profile::resolve_profile());
        if is_under_host_data_dir(Path::new(path_str), &data_dir) {
            return Err(host_data_dir_not_in_container_error(path_str, &workdir));
        }
        return Err(format!(
            "Docker container path '{path_str}' is outside {workdir}. Shell commands may access it, but {OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER} {workdir} paths to the host workspace."
        ));
    };

    Ok(Some(host_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_is_stable() {
        assert!(OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER.contains("workspace file tools only map"));
    }

    #[test]
    fn data_dir_paths_are_detected_lexically() {
        let data_dir = Path::new("/home/u/.local/share/com.fritzprix.libragent");
        assert!(is_under_host_data_dir(
            Path::new("/home/u/.local/share/com.fritzprix.libragent/system_skills/x/run.py"),
            data_dir
        ));
        assert!(!is_under_host_data_dir(Path::new("/tmp/x"), data_dir));
        assert!(!is_under_host_data_dir(
            Path::new("/home/u/.local/share/com.fritzprix.libragent-demo/x"),
            data_dir
        ));
    }

    #[test]
    fn data_dir_error_steers_to_import_not_shell() {
        let err = host_data_dir_not_in_container_error("/data/system_skills/a.py", "/app");
        assert!(err.contains(HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER));
        assert!(err.contains("workspace__importFiles"));
        assert!(!err.contains(OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER));
    }
}

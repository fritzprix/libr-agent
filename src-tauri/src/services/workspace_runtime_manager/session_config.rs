use std::path::{Path, PathBuf};

use crate::models::workspace_isolation::DEFAULT_DOCKER_WORKDIR;
use crate::repositories::SessionMetadata;
use crate::session_isolation::PathMappingLayer;

use super::error::{RuntimeResult, WorkspaceRuntimeError};

pub(super) fn docker_container_name(session: &SessionMetadata) -> RuntimeResult<String> {
    session
        .docker_container_name
        .clone()
        .or_else(|| Some(format!("libragent-session-{}", session.id)))
        .ok_or_else(|| WorkspaceRuntimeError::MissingContainerName(session.id.clone()))
}

pub(super) fn docker_host_workspace_path(session: &SessionMetadata) -> RuntimeResult<PathBuf> {
    session
        .docker_host_workspace_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| WorkspaceRuntimeError::MissingHostWorkspacePath(session.id.clone()))
}

pub(super) fn session_is_attach(session: &SessionMetadata) -> bool {
    session
        .docker_config
        .as_ref()
        .is_some_and(|config| config.is_attach())
}

pub(super) fn session_manage_lifecycle(session: &SessionMetadata) -> bool {
    session
        .docker_config
        .as_ref()
        .map(|config| config.manage_lifecycle())
        .unwrap_or(true)
}

pub(crate) fn session_docker_workdir(session: &SessionMetadata) -> String {
    session
        .docker_config
        .as_ref()
        .map(|config| config.workdir().to_string())
        .unwrap_or_else(|| DEFAULT_DOCKER_WORKDIR.to_string())
}

pub(super) fn path_mapper_for_session(
    session: &SessionMetadata,
    host_workspace: PathBuf,
) -> PathMappingLayer {
    PathMappingLayer::with_container_root(host_workspace, session_docker_workdir(session))
}

pub(super) fn docker_mount_path(path: &Path) -> RuntimeResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| WorkspaceRuntimeError::InvalidWorkspacePath(path.display().to_string()))
}

pub fn build_docker_volume_args(
    host_workspace: &Path,
    teamwork_dir: Option<&Path>,
) -> RuntimeResult<Vec<String>> {
    let mut args = Vec::new();
    let main_mount = format!("{}:/workspace", docker_mount_path(host_workspace)?);
    args.push("-v".to_string());
    args.push(main_mount);

    if let Some(tw_dir) = teamwork_dir {
        let tw_mount = format!(
            "{}:/workspace/.libragent/teamwork",
            docker_mount_path(tw_dir)?
        );
        args.push("-v".to_string());
        args.push(tw_mount);
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_docker_volume_args() {
        let host_workspace = Path::new("/home/user/workspace");

        // Test case 1: No teamwork directory
        let args = build_docker_volume_args(host_workspace, None).unwrap();
        assert!(args.contains(&"-v".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg.contains("/home/user/workspace:/workspace")));

        // Test case 2: With teamwork directory
        let teamwork_dir = Path::new("/home/user/.libragent/teamwork-artifacts/123");
        let args = build_docker_volume_args(host_workspace, Some(teamwork_dir)).unwrap();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "-v");
        assert!(args[1].contains("/home/user/workspace:/workspace"));
        assert_eq!(args[2], "-v");
        assert!(args[3].contains(
            "/home/user/.libragent/teamwork-artifacts/123:/workspace/.libragent/teamwork"
        ));
    }
}

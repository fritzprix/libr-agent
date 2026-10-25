//! Shared Docker container ↔ host path mapping for file-oriented tools
//! (workspace, media, …).

use std::path::{Component, Path, PathBuf};

use super::path_mapper::PathMappingLayer;
use crate::models::workspace_isolation::{WorkspaceIsolationMode, DEFAULT_DOCKER_WORKDIR};
use crate::repositories::SessionRepository;
use crate::services::skill_service::{self, SYSTEM_SKILLS_ALIAS_PREFIX, USER_SKILLS_ALIAS_PREFIX};

/// Stable substring in the outside-workdir Docker mapping error.
pub const OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER: &str = "workspace file tools only map";

/// Stable substring in the error for LibrAgent host data-dir paths, which exist
/// on the host only and are invisible to both file tools and the container shell.
pub const HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER: &str = "is a LibrAgent host path";

/// In-container copies of the host system/user skill roots in Docker sessions.
pub const CONTAINER_SYSTEM_SKILLS_DIR: &str = "/opt/libragent/skills/system";
pub const CONTAINER_USER_SKILLS_DIR: &str = "/opt/libragent/skills/user";

fn container_skill_root(alias_prefix: &str) -> Option<&'static str> {
    match alias_prefix {
        SYSTEM_SKILLS_ALIAS_PREFIX => Some(CONTAINER_SYSTEM_SKILLS_DIR),
        USER_SKILLS_ALIAS_PREFIX => Some(CONTAINER_USER_SKILLS_DIR),
        _ => None,
    }
}

/// Container path for a `@system-skills/…` or `@user-skills/…` alias path.
pub fn container_path_for_skill_alias(alias_path: &str) -> Option<String> {
    let (prefix, relative) = skill_service::extract_skill_alias_relative_path(alias_path)?;
    let root = container_skill_root(prefix)?;
    Some(if relative == "." {
        root.to_string()
    } else {
        format!("{root}/{relative}")
    })
}

/// Host path for a path under a container skill root; `..` components are rejected.
fn container_skill_path_to_host(
    path: &Path,
    system_dir: &Path,
    user_dir: &Path,
) -> Option<PathBuf> {
    [
        (CONTAINER_SYSTEM_SKILLS_DIR, system_dir),
        (CONTAINER_USER_SKILLS_DIR, user_dir),
    ]
    .into_iter()
    .find_map(|(container_root, host_root)| {
        let relative = path.strip_prefix(container_root).ok()?;
        if relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return None;
        }
        Some(host_root.join(relative))
    })
}

fn is_under_host_data_dir(path: &Path, data_dir: &Path) -> bool {
    path.starts_with(data_dir)
}

fn host_data_dir_not_in_container_error(path_str: &str) -> String {
    format!(
        "'{path_str}' {HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER} (data/skills directory) and does not exist in the container. Read skill files through the @system-skills/… or @user-skills/… alias; in workspace__runShell use {CONTAINER_SYSTEM_SKILLS_DIR}/… or {CONTAINER_USER_SKILLS_DIR}/…."
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
        if let (Ok(system_dir), Ok(user_dir)) = (
            skill_service::get_system_skills_directory(),
            skill_service::get_user_skills_directory(),
        ) {
            if let Some(host_path) =
                container_skill_path_to_host(Path::new(path_str), &system_dir, &user_dir)
            {
                return Ok(Some(host_path));
            }
        }
        let data_dir = crate::profile::data_dir(crate::profile::resolve_profile());
        if is_under_host_data_dir(Path::new(path_str), &data_dir) {
            return Err(host_data_dir_not_in_container_error(path_str));
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
    fn data_dir_error_steers_to_container_skill_paths() {
        let err = host_data_dir_not_in_container_error("/data/system_skills/a.py");
        assert!(err.contains(HOST_DATA_DIR_NOT_IN_CONTAINER_MARKER));
        assert!(err.contains(CONTAINER_SYSTEM_SKILLS_DIR));
        assert!(!err.contains(OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER));
    }

    #[test]
    fn skill_aliases_map_to_container_skill_roots() {
        assert_eq!(
            container_path_for_skill_alias("@system-skills/skill-deployer/scripts/deploy_skill.py")
                .as_deref(),
            Some("/opt/libragent/skills/system/skill-deployer/scripts/deploy_skill.py")
        );
        assert_eq!(
            container_path_for_skill_alias("@user-skills").as_deref(),
            Some(CONTAINER_USER_SKILLS_DIR)
        );
        assert_eq!(
            container_path_for_skill_alias("@workspace-skills/x/SKILL.md"),
            None
        );
        assert_eq!(container_path_for_skill_alias("scripts/run.py"), None);
    }

    #[test]
    fn container_skill_paths_map_to_host_roots() {
        let system_dir = Path::new("/data/system_skills");
        let user_dir = Path::new("/data/user_skills");
        assert_eq!(
            container_skill_path_to_host(
                Path::new("/opt/libragent/skills/system/a/SKILL.md"),
                system_dir,
                user_dir
            ),
            Some(PathBuf::from("/data/system_skills/a/SKILL.md"))
        );
        assert_eq!(
            container_skill_path_to_host(
                Path::new("/opt/libragent/skills/user"),
                system_dir,
                user_dir
            ),
            Some(PathBuf::from("/data/user_skills"))
        );
        assert_eq!(
            container_skill_path_to_host(
                Path::new("/opt/libragent/skills/system/../../../etc/passwd"),
                system_dir,
                user_dir
            ),
            None
        );
        assert_eq!(
            container_skill_path_to_host(
                Path::new("/opt/libragent/skills/systemx/a"),
                system_dir,
                user_dir
            ),
            None
        );
    }
}

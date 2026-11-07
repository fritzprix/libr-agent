use std::path::{Path, PathBuf};

use crate::models::workspace_isolation::DEFAULT_DOCKER_WORKDIR;
use crate::repositories::SessionMetadata;
use crate::session_isolation::{
    PathMappingLayer, CONTAINER_SYSTEM_SKILLS_DIR, CONTAINER_USER_SKILLS_DIR,
};

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

/// Refuses a managed workspace that is the user's home directory or an ancestor of it. A root
/// command in the container could then change dotfiles and other files of the whole home.
/// Symlinks are resolved when the paths exist.
pub(crate) fn refuse_home_workspace(
    host_workspace: &Path,
    home: Option<&Path>,
) -> RuntimeResult<()> {
    let Some(home) = home else {
        return Ok(());
    };
    let resolve = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if resolve(home).starts_with(resolve(host_workspace)) {
        return Err(WorkspaceRuntimeError::InvalidConfig(format!(
            "The workspace '{}' is your home directory or contains it. A container workspace \
             must not include your home directory. Select a project folder as the workspace.",
            host_workspace.display()
        )));
    }
    Ok(())
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

/// Docker host bind source (`-v` source or `docker cp` source).
///
/// Rust `canonicalize()` on Windows yields verbatim paths (`\\?\C:\...`), which
/// Docker rejects as bind sources. Strip the prefix at this argument boundary so
/// every bind source (workspace, teamwork, skills) is normalized consistently.
/// Verbatim UNC (`\\?\UNC\...`) is left unchanged: network paths need the
/// verbatim form, and LibrAgent bind sources are local drive paths today.
pub(super) fn docker_mount_path(path: &Path) -> RuntimeResult<String> {
    let raw = path
        .to_str()
        .ok_or_else(|| WorkspaceRuntimeError::InvalidWorkspacePath(path.display().to_string()))?;
    Ok(crate::mcp::builtin::utils::strip_windows_verbatim_prefix(raw).to_string())
}

/// The host workspace is mounted at the session workdir, which is also the root
/// that file tools map to the host workspace.
pub fn build_docker_volume_args(
    host_workspace: &Path,
    workdir: &str,
    teamwork_dir: Option<&Path>,
) -> RuntimeResult<Vec<String>> {
    let workdir = workdir.trim_end_matches('/');
    let mut args = Vec::new();
    let main_mount = format!("{}:{workdir}", docker_mount_path(host_workspace)?);
    args.push("-v".to_string());
    args.push(main_mount);

    if let Some(tw_dir) = teamwork_dir {
        let tw_mount = format!(
            "{}:{workdir}/.libragent/teamwork",
            docker_mount_path(tw_dir)?
        );
        args.push("-v".to_string());
        args.push(tw_mount);
    }

    Ok(args)
}

/// Bind mounts that expose the host skill roots at the container skill paths
/// shown in skill references. Both are read-only: user skills are written only
/// by `skill.py deploy`, which hands the skill to the host through the workspace
/// skill outbox (see `skill_service::install_outbox`).
pub fn build_skill_volume_args(
    system_dir: Option<&Path>,
    user_dir: Option<&Path>,
) -> RuntimeResult<Vec<String>> {
    let mut args = Vec::new();
    if let Some(dir) = system_dir {
        args.push("-v".to_string());
        args.push(format!(
            "{}:{CONTAINER_SYSTEM_SKILLS_DIR}:ro",
            docker_mount_path(dir)?
        ));
    }
    if let Some(dir) = user_dir {
        args.push("-v".to_string());
        args.push(format!(
            "{}:{CONTAINER_USER_SKILLS_DIR}:ro",
            docker_mount_path(dir)?
        ));
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_directory_and_its_ancestors_are_refused_as_a_managed_workspace() {
        let root = tempfile::tempdir().expect("root");
        let home = root.path().join("users").join("alice");
        let project = home.join("projects").join("app");
        std::fs::create_dir_all(&project).expect("dirs");

        for workspace in [home.as_path(), home.parent().expect("users"), root.path()] {
            let error = refuse_home_workspace(workspace, Some(&home))
                .expect_err("home or an ancestor must be refused")
                .to_string();
            assert!(error.contains("Select a project folder"), "{error}");
        }
        assert!(refuse_home_workspace(&project, Some(&home)).is_ok());
        assert!(refuse_home_workspace(&root.path().join("other"), Some(&home)).is_ok());
        assert!(refuse_home_workspace(&home, None).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_the_home_directory_is_refused() {
        let root = tempfile::tempdir().expect("root");
        let home = root.path().join("alice");
        std::fs::create_dir_all(&home).expect("home");
        let link = root.path().join("ws");
        std::os::unix::fs::symlink(&home, &link).expect("symlink");
        assert!(refuse_home_workspace(&link, Some(&home)).is_err());
    }

    #[test]
    fn skill_volume_args_mount_system_and_user_read_only() {
        let args = build_skill_volume_args(
            Some(Path::new("/data/system_skills")),
            Some(Path::new("/data/user_skills")),
        )
        .unwrap();
        assert_eq!(
            args,
            vec![
                "-v",
                "/data/system_skills:/opt/libragent/skills/system:ro",
                "-v",
                "/data/user_skills:/opt/libragent/skills/user:ro",
            ]
        );
        assert!(build_skill_volume_args(None, None).unwrap().is_empty());
    }

    #[test]
    fn test_build_docker_volume_args() {
        let host_workspace = Path::new("/home/user/workspace");

        // Test case 1: No teamwork directory
        let args = build_docker_volume_args(host_workspace, "/workspace", None).unwrap();
        assert!(args.contains(&"-v".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg.contains("/home/user/workspace:/workspace")));

        // Test case 2: With teamwork directory
        let teamwork_dir = Path::new("/home/user/.libragent/teamwork-artifacts/123");
        let args =
            build_docker_volume_args(host_workspace, "/workspace", Some(teamwork_dir)).unwrap();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "-v");
        assert!(args[1].contains("/home/user/workspace:/workspace"));
        assert_eq!(args[2], "-v");
        assert!(args[3].contains(
            "/home/user/.libragent/teamwork-artifacts/123:/workspace/.libragent/teamwork"
        ));
    }

    #[test]
    fn custom_workdir_is_the_workspace_mount_point() {
        let teamwork_dir = Path::new("/tw/123");
        let args =
            build_docker_volume_args(Path::new("/host/ws"), "/app/", Some(teamwork_dir)).unwrap();
        assert_eq!(
            args,
            vec![
                "-v",
                "/host/ws:/app",
                "-v",
                "/tw/123:/app/.libragent/teamwork"
            ]
        );
    }

    #[test]
    fn docker_mount_path_strips_windows_verbatim_prefixes() {
        assert_eq!(
            docker_mount_path(Path::new(r"C:\Temp\ws")).unwrap(),
            r"C:\Temp\ws"
        );
        assert_eq!(
            docker_mount_path(Path::new(r"\\?\C:\Temp\ws")).unwrap(),
            r"C:\Temp\ws"
        );
        assert_eq!(
            docker_mount_path(Path::new("//?/C:/Temp/ws")).unwrap(),
            "C:/Temp/ws"
        );
        // Verbatim UNC keeps the prefix: network paths need the verbatim form.
        assert_eq!(
            docker_mount_path(Path::new(r"\\?\UNC\server\share")).unwrap(),
            r"\\?\UNC\server\share"
        );
    }

    #[test]
    fn volume_args_strip_verbatim_prefix_from_windows_bind_sources() {
        let args = build_docker_volume_args(
            Path::new(r"\\?\C:\Temp\Rumi-E2E\O2-docker"),
            "/workspace",
            Some(Path::new(r"\\?\C:\Users\dev\.libragent\teamwork")),
        )
        .unwrap();
        assert_eq!(
            args,
            vec![
                "-v",
                r"C:\Temp\Rumi-E2E\O2-docker:/workspace",
                "-v",
                r"C:\Users\dev\.libragent\teamwork:/workspace/.libragent/teamwork",
            ]
        );
    }

    #[test]
    fn volume_args_keep_spaces_in_windows_bind_sources() {
        // Volume arguments go to docker as separate argv entries, so spaces stay literal.
        let args =
            build_docker_volume_args(Path::new(r"\\?\C:\Temp\My Workspace"), "/workspace", None)
                .unwrap();
        assert_eq!(args[1], r"C:\Temp\My Workspace:/workspace");
    }

    #[test]
    fn skill_volume_args_strip_verbatim_prefix_from_bind_sources() {
        let args = build_skill_volume_args(
            Some(Path::new(r"\\?\C:\ProgramData\libragent\system_skills")),
            Some(Path::new(r"\\?\C:\Users\dev\.libragent\user_skills")),
        )
        .unwrap();
        assert_eq!(
            args,
            vec![
                "-v",
                r"C:\ProgramData\libragent\system_skills:/opt/libragent/skills/system:ro",
                "-v",
                r"C:\Users\dev\.libragent\user_skills:/opt/libragent/skills/user:ro",
            ]
        );
    }
}

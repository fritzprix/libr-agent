use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::models::workspace_isolation::DEFAULT_SESSION_IMAGE;
use crate::repositories::{SessionMetadata, SessionRepository};
use crate::services::skill_service;
use crate::session_isolation::{ShellType, CONTAINER_SYSTEM_SKILLS_DIR, CONTAINER_USER_SKILLS_DIR};
use tokio::process::Command as AsyncCommand;

use super::docker_cli::{
    apply_docker_cli_env, current_uid_gid, docker_container_exists, docker_container_has_label,
    docker_container_running, run_docker_status, run_status_command, verify_container_label,
    ROOT_HARDENING_LABEL, WAKE_HOST_LABEL,
};
use super::error::{RuntimeResult, WorkspaceRuntimeError};
use super::session_config::{
    build_docker_volume_args, build_skill_volume_args, docker_mount_path, refuse_home_workspace,
    session_docker_workdir, session_is_attach,
};
use super::DockerStepReporter;

/// `docker run` arguments that let the container reach the host (Linux needs the
/// explicit mapping; Docker Desktop provides the name already) and mark it as
/// created with them.
fn wake_host_run_args() -> [String; 3] {
    [
        format!(
            "--add-host={}:host-gateway",
            crate::server::wake::DOCKER_HOST_NAME
        ),
        "--label".to_string(),
        format!("{WAKE_HOST_LABEL}=1"),
    ]
}

/// `docker run` arguments that limit what a root `docker exec` (runShell `asRoot`) can leave
/// behind for the host, and mark the container as created with them:
/// - `no-new-privileges`: a setuid file cannot give root to a later non-root command.
/// - no `SETFCAP`: root cannot put file capabilities on a file in a host bind mount.
/// - no `MKNOD`: root cannot create a device node in a host bind mount.
///
/// A setuid bit and root ownership are still possible; `RootExecLease::finish` removes them.
pub(super) fn hardening_run_args() -> [String; 8] {
    [
        "--security-opt".to_string(),
        "no-new-privileges".to_string(),
        "--cap-drop".to_string(),
        "SETFCAP".to_string(),
        "--cap-drop".to_string(),
        "MKNOD".to_string(),
        "--label".to_string(),
        format!("{ROOT_HARDENING_LABEL}=1"),
    ]
}

pub(super) async fn ensure_bash_image_contract(
    session_id: &str,
    container_name: &str,
    session: &SessionMetadata,
    host_workspace: &Path,
    image: &str,
    reporter: Option<&DockerStepReporter>,
) -> RuntimeResult<ShellType> {
    if session_is_attach(session) {
        if !docker_container_exists(container_name).await? {
            return Err(WorkspaceRuntimeError::AttachContainerNotFound(
                container_name.to_string(),
            ));
        }
        if !docker_container_running(container_name).await? {
            if let Some(reporter) = reporter {
                reporter("Starting attached container");
            }
            run_docker_status(["start", container_name]).await?;
        }
        if let Some(reporter) = reporter {
            reporter("Verifying shell");
        }
        return ensure_supported_shell(session_id, container_name).await;
    }

    refuse_home_workspace(host_workspace, dirs::home_dir().as_deref())?;

    let mut container_exists = docker_container_exists(container_name).await?;
    if container_exists {
        verify_container_label(container_name, session_id).await?;
        if !docker_container_has_label(container_name, WAKE_HOST_LABEL).await?
            || !docker_container_has_label(container_name, ROOT_HARDENING_LABEL).await?
        {
            // Created before the host mapping (wake callbacks cannot reach the host) or before
            // the root-exec hardening existed.
            if let Some(reporter) = reporter {
                reporter("Recreating container");
            }
            run_docker_status(["rm", "-f", container_name]).await?;
            container_exists = false;
        }
    }

    if container_exists {
        if !docker_container_running(container_name).await? {
            if let Some(reporter) = reporter {
                reporter("Starting container");
            }
            run_docker_status(["start", container_name]).await?;
        }
    } else {
        let config = session
            .docker_config
            .as_ref()
            .ok_or(WorkspaceRuntimeError::MissingDockerConfig)?;
        config
            .validate()
            .map_err(WorkspaceRuntimeError::InvalidConfig)?;
        let image_ref = config.image_ref().ok_or_else(|| {
            WorkspaceRuntimeError::InvalidConfig(
                "Managed Docker sessions require an image".to_string(),
            )
        })?;

        tokio::fs::create_dir_all(host_workspace)
            .await
            .map_err(|e| {
                WorkspaceRuntimeError::Io(format!(
                    "Failed to create Docker host workspace '{}': {e}",
                    host_workspace.display()
                ))
            })?;

        if let Some(reporter) = reporter {
            reporter(&format!("Pulling image {image}"));
        }
        ensure_image_available(image_ref).await?;

        let root_session_id = resolve_teamwork_root_session_id(session).await;
        let session_manager =
            crate::session::get_session_manager().map_err(WorkspaceRuntimeError::Io)?;
        let teamwork_dir = session_manager
            .get_directory_service()
            .get_teamwork_artifact_dir_unverified(&root_session_id);

        if session.org_id.is_some() || session.parent_session_id.is_some() {
            let _ = tokio::fs::create_dir_all(&teamwork_dir).await;
        }

        let tw_dir_arg = if teamwork_dir.exists() {
            Some(teamwork_dir.as_path())
        } else {
            None
        };

        let workdir = session_docker_workdir(session);
        let mut volume_args = build_docker_volume_args(host_workspace, &workdir, tw_dir_arg)?;
        let (system_skills_dir, user_skills_dir) = host_skill_dirs();
        if let Some(dir) = &user_skills_dir {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        volume_args.extend(build_skill_volume_args(
            system_skills_dir.as_deref().filter(|dir| dir.is_dir()),
            user_skills_dir.as_deref().filter(|dir| dir.is_dir()),
        )?);
        let label = format!("com.libragent.session_id={session_id}");
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["run", "-d", "--name", container_name, "--label", &label]);
        cmd.args(wake_host_run_args());
        cmd.args(hardening_run_args());
        cmd.args(volume_args);
        cmd.args(["-w", &workdir]);

        if let Some(config) = &session.docker_config {
            append_port_binding_args(&mut cmd, config).await?;
        }

        if let Some(user) = current_uid_gid().await {
            cmd.args(["--user", &user]);
            if !image_home_is_writable(image_ref, &user).await {
                cmd.args(["-e", &format!("HOME={FALLBACK_CONTAINER_HOME}")]);
            }
        }

        if let Some(reporter) = reporter {
            reporter("Starting container");
        }

        cmd.args([image_ref, "tail", "-f", "/dev/null"]);
        run_status_command(cmd).await?;
    }

    verify_container_label(container_name, session_id).await?;
    if let Some(reporter) = reporter {
        reporter("Verifying shell");
    }
    ensure_supported_shell(session_id, container_name).await
}

async fn ensure_image_available(image_ref: &str) -> RuntimeResult<()> {
    if run_docker_status(["image", "inspect", image_ref])
        .await
        .is_ok()
    {
        return Ok(());
    }
    run_docker_status(["pull", image_ref])
        .await
        .map_err(|error| match error {
            WorkspaceRuntimeError::DockerCommandFailed(details) => {
                let message = image_pull_failure(image_ref, &details);
                if is_image_missing(&details) {
                    WorkspaceRuntimeError::ImageNotFound(message)
                } else {
                    WorkspaceRuntimeError::DockerCommandFailed(message)
                }
            }
            other => other,
        })
}

/// Registry answers that mean the image does not exist (or is not visible to this user).
/// Network and daemon errors do not match, so they stay retryable.
fn is_image_missing(details: &str) -> bool {
    const MISSING_MARKERS: [&str; 4] = [
        "manifest unknown",
        "repository does not exist",
        "pull access denied",
        "invalid reference format",
    ];
    let details = details.to_ascii_lowercase();
    MISSING_MARKERS
        .iter()
        .any(|marker| details.contains(marker))
}

fn image_pull_failure(image_ref: &str, details: &str) -> String {
    if image_ref == DEFAULT_SESSION_IMAGE {
        format!(
            "Cannot pull the default LibrAgent session image '{image_ref}': {details}. \
             This image is published only for released LibrAgent versions. \
             Set a different Docker image in the session isolation settings."
        )
    } else {
        format!("Cannot pull Docker image '{image_ref}': {details}")
    }
}

const FALLBACK_CONTAINER_HOME: &str = "/tmp";

/// Running as the host uid leaves `HOME=/` when the image has no passwd entry for
/// it, and tools that write under `~` (pip, npm caches) then fail.
async fn image_home_is_writable(image_ref: &str, user: &str) -> bool {
    run_docker_status([
        "run",
        "--rm",
        "--user",
        user,
        "--entrypoint",
        "sh",
        image_ref,
        "-c",
        r#"[ "$HOME" != / ] && [ -w "$HOME" ]"#,
    ])
    .await
    .is_ok()
}

fn host_skill_dirs() -> (Option<PathBuf>, Option<PathBuf>) {
    (
        skill_service::get_system_skills_directory().ok(),
        skill_service::get_user_skills_directory().ok(),
    )
}

/// Copies host skill roots into containers that lack them: attach containers
/// (mounts cannot be added) and managed containers created before skill mounts.
/// Best effort — a missing skill tree must not block the session.
pub(super) async fn stage_missing_skill_roots(container_name: &str) {
    let (system_dir, user_dir) = host_skill_dirs();
    for (host_dir, container_dir) in [
        (system_dir, CONTAINER_SYSTEM_SKILLS_DIR),
        (user_dir, CONTAINER_USER_SKILLS_DIR),
    ] {
        let Some(host_dir) = host_dir.filter(|dir| dir.is_dir()) else {
            continue;
        };
        if run_docker_status(["exec", container_name, "test", "-d", container_dir])
            .await
            .is_ok()
        {
            continue;
        }
        if let Err(error) = copy_skill_root(container_name, &host_dir, container_dir).await {
            tracing::warn!(
                "Failed to copy skills '{}' into container {container_name}:{container_dir}: {error}",
                host_dir.display()
            );
        }
    }
}

async fn copy_skill_root(
    container_name: &str,
    host_dir: &Path,
    container_dir: &str,
) -> RuntimeResult<()> {
    let parent = Path::new(container_dir)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("/");
    run_docker_status(["exec", "-u", "0", container_name, "mkdir", "-p", parent]).await?;
    let source = format!("{}/.", docker_mount_path(host_dir)?);
    let destination = format!("{container_name}:{container_dir}");
    run_docker_status(["cp", &source, &destination]).await
}

pub(super) async fn append_port_binding_args(
    cmd: &mut AsyncCommand,
    config: &crate::models::workspace_isolation::DockerWorkspaceConfig,
) -> RuntimeResult<()> {
    for binding in &config.port_bindings {
        if let Some(host_port) = binding.host_port {
            ensure_host_port_available(host_port)?;
        }

        let published = match binding.host_port {
            Some(host_port) => format!("127.0.0.1:{host_port}:{}", binding.container_port),
            None => format!("127.0.0.1::{}", binding.container_port),
        };
        cmd.arg("-p").arg(published);
    }

    Ok(())
}

pub(super) fn ensure_host_port_available(port: u16) -> RuntimeResult<()> {
    TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .map_err(|_| WorkspaceRuntimeError::HostPortUnavailable(port))
}

pub(super) async fn ensure_supported_shell(
    session_id: &str,
    container_name: &str,
) -> RuntimeResult<ShellType> {
    if run_docker_status(["exec", container_name, "bash", "-lc", "true"])
        .await
        .is_ok()
    {
        return Ok(ShellType::Bash);
    }

    if run_docker_status(["exec", container_name, "sh", "-lc", "true"])
        .await
        .is_ok()
    {
        return Ok(ShellType::Sh);
    }

    Err(WorkspaceRuntimeError::ShellUnavailable {
        session_id: session_id.to_string(),
    })
}

pub(super) async fn resolve_teamwork_root_session_id(session: &SessionMetadata) -> String {
    if let Some(org_root_session_id) = &session.org_root_session_id {
        return org_root_session_id.clone();
    }

    if let Some(session_repo) = crate::state::try_get_session_repository() {
        let mut current = session.clone();
        for _ in 0..64 {
            let Some(parent_session_id) = current.parent_session_id.clone() else {
                return current.id;
            };

            match session_repo.get_session(&parent_session_id).await {
                Ok(Some(parent_session)) => {
                    current = parent_session;
                    if let Some(org_root_session_id) = &current.org_root_session_id {
                        return org_root_session_id.clone();
                    }
                }
                _ => return parent_session_id,
            }
        }
        current.id
    } else {
        session.id.clone()
    }
}

#[cfg(test)]
mod wake_host_tests {
    use super::*;

    #[test]
    fn run_args_map_the_host_name_and_mark_the_container() {
        assert_eq!(
            wake_host_run_args(),
            [
                "--add-host=host.docker.internal:host-gateway",
                "--label",
                "com.libragent.wake_host=1"
            ]
        );
    }

    #[test]
    fn hardening_blocks_new_privileges_file_capabilities_and_device_nodes() {
        assert_eq!(
            hardening_run_args(),
            [
                "--security-opt",
                "no-new-privileges",
                "--cap-drop",
                "SETFCAP",
                "--cap-drop",
                "MKNOD",
                "--label",
                "com.libragent.root_hardening=1"
            ]
        );
    }
}

#[cfg(test)]
mod image_pull_tests {
    use super::*;

    #[test]
    fn default_image_pull_failure_names_the_image_and_the_override() {
        let message = image_pull_failure(DEFAULT_SESSION_IMAGE, "manifest unknown");
        assert!(message.contains(DEFAULT_SESSION_IMAGE), "{message}");
        assert!(message.contains("manifest unknown"), "{message}");
        assert!(message.contains("session isolation settings"), "{message}");
    }

    #[test]
    fn custom_image_pull_failure_names_the_image_only() {
        let message = image_pull_failure("python:3.12-slim", "denied");
        assert!(message.contains("'python:3.12-slim'"), "{message}");
        assert!(!message.contains("settings"), "{message}");
    }

    #[test]
    fn only_registry_not_found_answers_count_as_a_missing_image() {
        for missing in [
            "Error response from daemon: manifest for foo:1 not found: manifest unknown",
            "Error response from daemon: pull access denied for nosuch, repository does not exist or may require 'docker login'",
            "invalid reference format: repository name must be lowercase",
        ] {
            assert!(is_image_missing(missing), "{missing}");
        }
        for transient in [
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?",
            "Get \"https://registry-1.docker.io/v2/\": dial tcp: lookup registry-1.docker.io: no such host",
            "net/http: TLS handshake timeout",
            "toomanyrequests: You have reached your pull rate limit",
        ] {
            assert!(!is_image_missing(transient), "{transient}");
        }
    }
}

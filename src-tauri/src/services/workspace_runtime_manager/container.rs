use std::net::TcpListener;
use std::path::Path;

use crate::repositories::{SessionMetadata, SessionRepository};
use crate::session_isolation::ShellType;
use tokio::process::Command as AsyncCommand;

use super::docker_cli::{
    apply_docker_cli_env, current_uid_gid, docker_container_exists, docker_container_running,
    run_docker_status, run_status_command, verify_container_label,
};
use super::error::{RuntimeResult, WorkspaceRuntimeError};
use super::session_config::{build_docker_volume_args, session_docker_workdir, session_is_attach};
use super::DockerStepReporter;

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
            return Err(WorkspaceRuntimeError::DockerCommandFailed(format!(
                "Attach container '{container_name}' was not found"
            )));
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

    if docker_container_exists(container_name).await? {
        verify_container_label(container_name, session_id).await?;
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

        let volume_args = build_docker_volume_args(host_workspace, tw_dir_arg)?;
        let label = format!("com.libragent.session_id={session_id}");
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["run", "-d", "--name", container_name, "--label", &label]);
        cmd.args(volume_args);
        cmd.args(["-w", &workdir]);

        if let Some(config) = &session.docker_config {
            append_port_binding_args(&mut cmd, config).await?;
        }

        if let Some(user) = current_uid_gid().await {
            cmd.args(["--user", &user]);
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

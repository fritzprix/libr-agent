//! Container lifecycle cleanup on the manager: per-session removal and the
//! startup sweep of containers whose sessions are gone.

use std::collections::HashSet;

use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionMetadata;

use super::docker_cli::{
    docker_container_exists, docker_container_label, docker_output_slice, run_docker_status,
    verify_container_label,
};
use super::error::RuntimeResult;
use super::provisioning::clear_session_docker_caches;
use super::session_config::{docker_container_name, session_manage_lifecycle};
use super::WorkspaceRuntimeManager;

impl WorkspaceRuntimeManager {
    pub async fn remove_runtime_for_session(session: &SessionMetadata) -> RuntimeResult<()> {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Ok(());
        }

        clear_session_docker_caches(&session.id);

        if !session_manage_lifecycle(session) {
            return Ok(());
        }

        Self::healthcheck().await?;
        let container_name = docker_container_name(session)?;
        if !docker_container_exists(&container_name).await? {
            return Ok(());
        }

        verify_container_label(&container_name, &session.id).await?;
        run_docker_status(["rm", "-f", "-v", &container_name]).await
    }

    pub async fn sweep_stale_containers(
        active_session_ids: &HashSet<String>,
    ) -> RuntimeResult<Vec<String>> {
        Self::healthcheck().await?;

        let output = docker_output_slice(&[
            "ps",
            "-a",
            "--filter",
            "label=com.libragent.session_id",
            "--format",
            "{{.Names}}",
        ])
        .await?;

        let mut removed = Vec::new();
        for container_name in output
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            let session_id = docker_container_label(container_name).await?;
            if active_session_ids.contains(&session_id) {
                continue;
            }

            verify_container_label(container_name, &session_id).await?;
            if let Err(error) = run_docker_status(["rm", "-f", "-v", container_name]).await {
                tracing::warn!(
                    "Failed to remove stale Docker container '{}': {}",
                    container_name,
                    error
                );
                continue;
            }
            removed.push(container_name.to_string());
        }

        Ok(removed)
    }
}

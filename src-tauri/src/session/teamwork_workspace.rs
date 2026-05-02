use std::path::PathBuf;

use crate::repositories::SessionRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamworkWorkspaceStatus {
    pub effective_workspace: PathBuf,
    pub dedicated_workspace: PathBuf,
}

impl TeamworkWorkspaceStatus {
    pub fn uses_dedicated_teamwork_workspace(&self) -> bool {
        self.effective_workspace == self.dedicated_workspace
    }
}

pub fn teamwork_workspace_status(
    session_manager: &crate::session::SessionManager,
    session_id: &str,
) -> TeamworkWorkspaceStatus {
    TeamworkWorkspaceStatus {
        effective_workspace: session_manager.get_session_workspace_dir_by_id(session_id),
        dedicated_workspace: session_manager
            .get_directory_service()
            .get_teamwork_workspace_dir_unverified(session_id),
    }
}

/// Provision the dedicated teamwork workspace for a governing/root session and
/// persist it as that session's effective workspace override.
pub async fn provision_teamwork_workspace_for_session(
    session_repo: &dyn SessionRepository,
    session_manager: &crate::session::SessionManager,
    session_id: &str,
) -> Result<PathBuf, String> {
    let teamwork_workspace = session_manager
        .get_directory_service()
        .create_teamwork_workspace(session_id)
        .await?;

    let teamwork_workspace_str = teamwork_workspace
        .to_str()
        .ok_or_else(|| {
            format!(
                "Invalid teamwork workspace path encoding: {}",
                teamwork_workspace.display()
            )
        })?
        .to_string();

    session_repo
        .update_workspace_override(session_id, Some(teamwork_workspace_str))
        .await
        .map_err(|e| format!("Failed to persist teamwork workspace override: {}", e))?;

    session_manager
        .register_session_override(session_id, teamwork_workspace.clone())
        .await?;

    Ok(teamwork_workspace)
}

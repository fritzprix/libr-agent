use crate::models::workspace_isolation::{DockerWorkspaceConfig, WorkspaceIsolationMode};
use crate::repositories::SessionMetadata;

use super::super::enrichment::{display_sanitize_workspace_path, WorkspaceRelation};

/// Inherit Docker *attach* isolation from an org root for org children.
///
/// Org children already share the org-root host workspace path. Harbor trials
/// attach the root to the trial container (`attachContainer` + workdir `/app`);
/// without inheriting that attach config, children write only to the host
/// staging dir while the verifier reads the container filesystem.
///
/// Managed (image-based) Docker is per-session and is not inherited — a second
/// container would not share the root's filesystem.
pub fn org_child_inherited_docker_workspace(
    org_root: &SessionMetadata,
) -> Option<(WorkspaceIsolationMode, DockerWorkspaceConfig)> {
    if org_root.workspace_isolation != WorkspaceIsolationMode::Docker {
        return None;
    }
    let config = org_root.docker_config.as_ref()?;
    if !config.is_attach() {
        return None;
    }
    let mut inherited = config.clone();
    // Never let a child stop/remove a Harbor (or other external) attach target.
    inherited.manage_lifecycle = Some(false);
    Some((WorkspaceIsolationMode::Docker, inherited))
}

/// Tokens that mean "use the caller's effective workspace root" (not a literal path).
pub fn is_inherit_workspace_override(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "current" | "." | "inherit"
    )
}

/// Resolve the child workspace path for `agent__spawnSession`.
///
/// - `workspaceOverride` of `current` / `.` / `inherit` → caller's workspace
/// - any other override string → used as the workspace path (absolute expected)
/// - no override + org root → org root workspace (existing behavior)
/// - no override + plain session → `None` (isolated child workspace)
pub fn resolve_spawn_workspace_binding(
    requested_override: Option<&str>,
    caller_workspace: &std::path::Path,
    org_root_workspace: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    if let Some(raw) = requested_override {
        let trimmed = raw.trim();
        if is_inherit_workspace_override(trimmed) {
            return Some(caller_workspace.to_path_buf());
        }
        return Some(std::path::PathBuf::from(trimmed));
    }
    org_root_workspace.map(std::path::Path::to_path_buf)
}

/// Resolve display path + SHARED/ISOLATED for a newly spawned child session.
pub(super) fn resolve_spawned_workspace_signal(
    caller_session_id: &str,
    child_session_id: &str,
    intended_workspace: Option<String>,
) -> (Option<String>, Option<WorkspaceRelation>) {
    let Ok(session_manager) = crate::session::get_session_manager() else {
        let display = intended_workspace
            .as_deref()
            .and_then(display_sanitize_workspace_path);
        return (display, None);
    };

    let child_raw = intended_workspace.unwrap_or_else(|| {
        session_manager
            .get_session_workspace_dir_by_id(child_session_id)
            .to_string_lossy()
            .into_owned()
    });
    let caller_raw = session_manager
        .get_session_workspace_dir_by_id(caller_session_id)
        .to_string_lossy()
        .into_owned();

    let relation = if crate::mcp::builtin::utils::workspace_paths_equivalent(
        std::path::Path::new(&child_raw),
        std::path::Path::new(&caller_raw),
    ) {
        WorkspaceRelation::Shared
    } else {
        WorkspaceRelation::Isolated
    };
    // Strip newlines before display — workspaceOverride is user-controlled and this
    // path lands in the plain-text spawnSession note (not only the Metadata fence).
    let display = display_sanitize_workspace_path(&child_raw);
    (display, Some(relation))
}

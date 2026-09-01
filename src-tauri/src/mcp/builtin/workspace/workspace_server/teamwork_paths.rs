use super::WorkspaceServer;
use crate::mcp::builtin::utils::{path_starts_with, relative_path_under_base};
use crate::repositories::SessionRepository;
use std::path::{Path, PathBuf};

const TEAMWORK_ALIAS_PREFIX: &str = "@teamwork";
// Guard against pathological or cyclic parent-session chains while still
// allowing deep enough org hierarchies for normal teamwork lineages.
const TEAMWORK_PARENT_CHAIN_LIMIT: usize = 64;

impl WorkspaceServer {
    pub(super) async fn get_allowed_absolute_skill_roots(
        &self,
        session_id: &str,
    ) -> Result<Vec<PathBuf>, String> {
        let assistant_id = if let Some(repo) = crate::state::try_get_session_repository() {
            repo.get_session(session_id)
                .await
                .map_err(|e| format!("Failed to load session metadata: {e}"))?
                .and_then(|session| crate::agent::extract_assistant_id_from_session(&session))
        } else {
            None
        };

        let workspace_dir = self.get_workspace_dir(session_id);
        let (system_dir, user_dir, assistant_dir, workspace_skill_dir) =
            crate::services::skill_service::resolve_skill_directories(
                assistant_id.as_deref(),
                Some(session_id),
                Some(&workspace_dir),
            )
            .await?;

        Ok(crate::services::skill_service::collect_allowed_skill_roots(
            system_dir,
            user_dir,
            assistant_dir,
            workspace_skill_dir,
        ))
    }

    pub(super) async fn get_skill_alias_roots(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::services::skill_service::SkillAliasRoot>, String> {
        let assistant_id = if let Some(repo) = crate::state::try_get_session_repository() {
            repo.get_session(session_id)
                .await
                .map_err(|e| format!("Failed to load session metadata: {e}"))?
                .and_then(|session| crate::agent::extract_assistant_id_from_session(&session))
        } else {
            None
        };

        let workspace_dir = self.get_workspace_dir(session_id);
        let (system_dir, user_dir, assistant_dir, workspace_skill_dir) =
            crate::services::skill_service::resolve_skill_directories(
                assistant_id.as_deref(),
                Some(session_id),
                Some(&workspace_dir),
            )
            .await?;

        Ok(crate::services::skill_service::collect_skill_alias_roots(
            system_dir,
            user_dir,
            assistant_dir,
            workspace_skill_dir,
        ))
    }

    pub(super) fn extract_teamwork_alias_relative_path(path_str: &str) -> Option<&str> {
        if path_str == TEAMWORK_ALIAS_PREFIX
            || path_str == ".libragent/teamwork"
            || path_str == ".libragent\\teamwork"
        {
            return Some(".");
        }

        path_str
            .strip_prefix("@teamwork/")
            .or_else(|| path_str.strip_prefix("@teamwork\\"))
            .or_else(|| path_str.strip_prefix(".libragent/teamwork/"))
            .or_else(|| path_str.strip_prefix(".libragent\\teamwork\\"))
            .map(|suffix| {
                if suffix.trim().is_empty() {
                    "."
                } else {
                    suffix
                }
            })
    }

    async fn resolve_teamwork_root_session_id(&self, session_id: &str) -> Result<String, String> {
        let Some(repo) = crate::state::try_get_session_repository() else {
            return Ok(session_id.to_string());
        };

        let mut current = match repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to load session metadata: {e}"))?
        {
            Some(session) => session,
            None => return Ok(session_id.to_string()),
        };

        if let Some(org_root_session_id) = current.org_root_session_id.clone() {
            return Ok(org_root_session_id);
        }

        for _ in 0..TEAMWORK_PARENT_CHAIN_LIMIT {
            let Some(parent_session_id) = current.parent_session_id.clone() else {
                return Ok(current.id);
            };

            current = match repo
                .get_session(&parent_session_id)
                .await
                .map_err(|e| format!("Failed to load parent session metadata: {e}"))?
            {
                Some(session) => session,
                None => return Ok(parent_session_id),
            };

            if let Some(org_root_session_id) = current.org_root_session_id.clone() {
                return Ok(org_root_session_id);
            }
        }

        Err(format!(
            "Failed to resolve teamwork root for session {}: parent chain exceeded {} hops or contains a cycle",
            session_id, TEAMWORK_PARENT_CHAIN_LIMIT
        ))
    }

    pub(super) async fn get_teamwork_artifact_root(
        &self,
        session_id: &str,
    ) -> Result<PathBuf, String> {
        let root_session_id = self.resolve_teamwork_root_session_id(session_id).await?;
        Ok(crate::session::teamwork_artifact_dir_for_session(
            &self.session_manager,
            &root_session_id,
        ))
    }

    pub(super) fn path_is_within_any_root(
        candidate_path: &Path,
        allowed_roots: &[PathBuf],
    ) -> bool {
        let normalized_candidate = candidate_path
            .canonicalize()
            .unwrap_or_else(|_| candidate_path.to_path_buf());

        allowed_roots.iter().any(|root| {
            let normalized_root = root.canonicalize().unwrap_or_else(|_| root.clone());
            path_starts_with(&normalized_candidate, &normalized_root)
        })
    }

    /// Map an absolute path under the teamwork artifact root to a scoped relative path.
    ///
    /// Handles new-file writes (path may not exist yet) and Windows/Unix canonicalize
    /// asymmetry by trying the raw root, the canonical root, and an existing-ancestor walk.
    pub(super) fn extract_absolute_teamwork_relative_path(
        candidate: &Path,
        teamwork_root: &Path,
    ) -> Option<String> {
        let canonical_root = teamwork_root.canonicalize().ok();

        if let Some(relative) = relative_path_under_base(candidate, teamwork_root) {
            return Some(Self::normalize_teamwork_relative_path(&relative));
        }

        if let Some(ref canonical_root) = canonical_root {
            if teamwork_root != canonical_root.as_path() {
                if let Some(relative) = relative_path_under_base(candidate, canonical_root) {
                    return Some(Self::normalize_teamwork_relative_path(&relative));
                }
            }
        }

        let canonical_root = canonical_root?;
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut current = candidate.to_path_buf();

        loop {
            match current.canonicalize() {
                Ok(canonical_current) => {
                    let relative = relative_path_under_base(&canonical_current, &canonical_root)?;
                    let mut full = relative;
                    for part in suffix.iter().rev() {
                        if part == ".." {
                            return None;
                        }
                        if part == "." {
                            continue;
                        }
                        full.push(part);
                    }
                    return Some(Self::normalize_teamwork_relative_path(&full));
                }
                Err(_) => {
                    let file_name = current.file_name()?.to_os_string();
                    suffix.push(file_name);
                    if !current.pop() {
                        return None;
                    }
                }
            }
        }
    }

    fn normalize_teamwork_relative_path(relative: &Path) -> String {
        if relative.as_os_str().is_empty() {
            ".".to_string()
        } else {
            relative.to_string_lossy().replace('\\', "/")
        }
    }
}

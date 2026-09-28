use super::WorkspaceServer;
use crate::SecureFileManager;
use std::path::PathBuf;

/// Marker substring for skill-alias write rejection (recovery guidance + tests).
pub const SKILL_ALIAS_WRITE_REJECTED_MARKER: &str = "Skill aliases are read-only";

fn skill_alias_write_rejected_error(alias_prefix: &str) -> String {
    format!(
        "{SKILL_ALIAS_WRITE_REJECTED_MARKER}: `{alias_prefix}` is a managed skill reference \
         for read/list only. Writing through skill aliases creates a literal `@…` directory \
         under the workspace that skill discovery does not scan. \
         Write workspace skills to `.libragent/skills/{{name}}/SKILL.md`, or deploy with \
         skill-deployer into `.libragent/skills/` / `user_skills/` (never `system_skills/`)."
    )
}

impl WorkspaceServer {
    pub async fn validate_read_path_with_skill_access(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        if let Some(teamwork_relative_path) = Self::extract_teamwork_alias_relative_path(path_str) {
            let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
            let teamwork_manager = SecureFileManager::new_scoped_with_base_dir(teamwork_root);
            return teamwork_manager
                .get_security_validator()
                .validate_path_for_read(teamwork_relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        if let Some((alias_prefix, relative_path)) =
            crate::services::skill_service::extract_skill_alias_relative_path(path_str)
        {
            let alias_roots = self.get_skill_alias_roots(&target_session_id).await?;
            let alias_root = alias_roots
                .into_iter()
                .find(|root| root.prefix == alias_prefix)
                .ok_or_else(|| format!("Skill alias root is not available: {alias_prefix}"))?;
            let alias_manager = SecureFileManager::new_scoped_with_base_dir(alias_root.root);
            return alias_manager
                .get_security_validator()
                .validate_path_for_read(relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        let mapped_path = self
            .map_docker_container_file_tool_path(path_str, &target_session_id)
            .await?;
        let effective_path = mapped_path.as_deref().unwrap_or(path_str);
        let file_manager = self.get_file_manager(Some(target_session_id.clone()));

        match file_manager
            .get_security_validator()
            .validate_path_for_read(effective_path)
        {
            Ok(path) => Ok(path),
            Err(original_error) => {
                let candidate_path = PathBuf::from(effective_path);
                if !candidate_path.is_absolute() {
                    return Err(format!("Security error: {original_error}"));
                }

                let allowed_roots = self
                    .get_allowed_absolute_skill_roots(&target_session_id)
                    .await?;
                let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
                let mut allowed_roots = allowed_roots;
                allowed_roots.push(teamwork_root);

                if !Self::path_is_within_any_root(&candidate_path, &allowed_roots) {
                    return Err(format!("Security error: {original_error}"));
                }

                let permissive_manager = SecureFileManager::new_with_base_dir(
                    self.get_workspace_dir(&target_session_id),
                );
                permissive_manager
                    .get_security_validator()
                    .validate_path_for_read(effective_path)
                    .map_err(|e| format!("Security error: {e}"))
            }
        }
    }

    pub async fn validate_write_path_with_teamwork_access(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        if let Some(teamwork_relative_path) = Self::extract_teamwork_alias_relative_path(path_str) {
            let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
            let teamwork_manager = SecureFileManager::new_scoped_with_base_dir(teamwork_root);
            return teamwork_manager
                .get_security_validator()
                .validate_path_for_write(teamwork_relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        // Skill aliases (@workspace-skills, @user-skills, …) are read/list-only.
        // Resolving them for write would blur deploy paths; treating them as
        // workspace-relative paths creates a literal `@…` directory (regression).
        if let Some((alias_prefix, _)) =
            crate::services::skill_service::extract_skill_alias_relative_path(path_str)
        {
            return Err(skill_alias_write_rejected_error(alias_prefix));
        }

        let mapped_path = self
            .map_docker_container_file_tool_path(path_str, &target_session_id)
            .await?;
        let effective_path = mapped_path.as_deref().unwrap_or(path_str);

        let candidate_path = PathBuf::from(effective_path);
        if candidate_path.is_absolute() {
            if let Ok(teamwork_root) = self.get_teamwork_artifact_root(&target_session_id).await {
                if let Some(relative_path) =
                    Self::extract_absolute_teamwork_relative_path(&candidate_path, &teamwork_root)
                {
                    let teamwork_manager =
                        SecureFileManager::new_scoped_with_base_dir(teamwork_root);
                    return teamwork_manager
                        .get_security_validator()
                        .validate_path_for_write(&relative_path)
                        .map_err(|e| format!("Security error: {e}"));
                }
            }
        }

        self.validate_path_with_error_for_write(effective_path, Some(target_session_id))
    }

    async fn map_docker_container_file_tool_path(
        &self,
        path_str: &str,
        target_session_id: &str,
    ) -> Result<Option<String>, String> {
        let mapped = crate::session_isolation::map_docker_container_file_tool_path(
            target_session_id,
            path_str,
        )
        .await?;
        Ok(mapped.map(|path| path.to_string_lossy().to_string()))
    }

    /// After a mutating file tool write, push staging → attach container when needed.
    pub async fn sync_attach_after_host_write(
        &self,
        host_path: &std::path::Path,
        session_id: Option<&str>,
    ) -> Result<(), String> {
        let target_session_id = session_id.unwrap_or(self.session_id.as_str());
        let Some(session) =
            crate::services::container_attach_fs::load_session(target_session_id).await?
        else {
            return Ok(());
        };
        crate::services::container_attach_fs::push_host_file_to_container(&session, host_path).await
    }

    /// Before reading a staged path, pull attach container → staging when needed.
    /// Propagates docker failures for workdir paths so tools do not read stale staging.
    pub async fn sync_attach_before_host_read(
        &self,
        host_path: &std::path::Path,
        session_id: Option<&str>,
    ) -> Result<(), String> {
        let target_session_id = session_id.unwrap_or(self.session_id.as_str());
        let Some(session) =
            crate::services::container_attach_fs::load_session(target_session_id).await?
        else {
            return Ok(());
        };
        crate::services::container_attach_fs::pull_container_file_to_host(&session, host_path).await
    }

    /// Validate path with security checks (helper for file operations)
    pub fn validate_path_with_error(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        super::super::file_operations::utils::validate_path_with_error(&file_manager, path_str)
    }

    /// Validate path for write/create operations.
    /// Blocks Windows reserved filenames in addition to standard security checks.
    /// Delete operations should use `validate_path_with_error` instead so that
    /// pre-existing reserved-name files can still be cleaned up.
    pub fn validate_path_with_error_for_write(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        super::super::file_operations::utils::validate_path_with_error_for_write(
            &file_manager,
            path_str,
        )
    }
}

/// Recovery for path-validation failures. Outside-workdir mapping errors steer to
/// `workspace__runShell` (file tools cannot map those paths); other failures keep
/// the caller's fallback (often `listDirectory`).
pub(crate) fn path_validation_failure_guidance(error: &str, fallback: Vec<String>) -> Vec<String> {
    if error.contains(crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER) {
        vec![
            "Use workspace__runShell for this container path (ls/cat to inspect; mkdir -p + redirect or cp to write).".to_string(),
            "Do not retry as a workspace-relative path — that targets a different location under the workdir.".to_string(),
        ]
    } else if error.contains(SKILL_ALIAS_WRITE_REJECTED_MARKER) {
        vec![
            "Write the skill to `.libragent/skills/<skill-name>/SKILL.md` (workspace) or use skill-deployer for `user_skills/` / `.libragent/skills/`.".to_string(),
            "Do not write through @workspace-skills / @user-skills / @assistant-skills / @system-skills — those aliases are read/list only.".to_string(),
            "After writing under `.libragent/skills/`, verify with workspace__readFile on the matching @workspace-skills/... alias.".to_string(),
        ]
    } else {
        fallback
    }
}

#[cfg(test)]
mod guidance_tests {
    use super::{
        path_validation_failure_guidance, skill_alias_write_rejected_error,
        SKILL_ALIAS_WRITE_REJECTED_MARKER,
    };
    use crate::services::skill_service::WORKSPACE_SKILLS_ALIAS_PREFIX;
    use crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER;

    #[test]
    fn outside_workdir_guidance_prefers_run_shell() {
        let err = format!("… {OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER} /app …");
        let guidance = path_validation_failure_guidance(
            &err,
            vec!["Use workspace__listDirectory to see available paths".to_string()],
        );
        assert!(guidance
            .iter()
            .any(|line| line.contains("workspace__runShell")));
        assert!(guidance
            .iter()
            .all(|line| !line.contains("workspace__listDirectory")));
    }

    #[test]
    fn skill_alias_write_guidance_points_at_libragent_skills() {
        let err = skill_alias_write_rejected_error(WORKSPACE_SKILLS_ALIAS_PREFIX);
        assert!(err.contains(SKILL_ALIAS_WRITE_REJECTED_MARKER));
        let guidance = path_validation_failure_guidance(
            &err,
            vec!["Use workspace__listDirectory to see available paths".to_string()],
        );
        assert!(guidance
            .iter()
            .any(|line| line.contains(".libragent/skills")));
        assert!(guidance.iter().any(|line| line.contains("read/list only")));
        assert!(guidance.iter().all(|line| !line.contains("listDirectory")));
    }

    #[test]
    fn other_errors_keep_fallback() {
        let fallback = vec!["Use workspace__listDirectory to see available paths".to_string()];
        let guidance =
            path_validation_failure_guidance("Security error: blocked", fallback.clone());
        assert_eq!(guidance, fallback);
    }
}

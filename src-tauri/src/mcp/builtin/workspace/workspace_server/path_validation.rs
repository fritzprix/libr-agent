use super::WorkspaceServer;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionRepository;
use crate::session_isolation::PathMappingLayer;
use crate::SecureFileManager;
use std::path::PathBuf;

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
        if !path_str.starts_with('/') {
            return Ok(None);
        }

        let Some(session_repo) = crate::state::try_get_session_repository() else {
            return Ok(None);
        };
        let Some(session) = session_repo
            .get_session(target_session_id)
            .await
            .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
        else {
            return Ok(None);
        };

        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Ok(None);
        }

        let host_workspace = session.docker_host_workspace_path.as_ref().ok_or_else(|| {
            format!("Missing Docker host workspace path for session {target_session_id}")
        })?;
        let workdir = session
            .docker_config
            .as_ref()
            .map(|config| config.workdir().to_string())
            .unwrap_or_else(|| {
                crate::models::workspace_isolation::DEFAULT_DOCKER_WORKDIR.to_string()
            });
        let mapper = PathMappingLayer::with_container_root(PathBuf::from(host_workspace), &workdir);
        let Some(host_path) = mapper.container_to_host(path_str) else {
            return Err(format!(
                "Docker container path '{path_str}' is outside {workdir}. Shell commands may access it, but workspace file tools only map {workdir} paths to the host workspace."
            ));
        };

        Ok(Some(host_path.to_string_lossy().to_string()))
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

use super::status::record_session_error_with_dispatcher;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{CompactContextRecord, SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Result of inspecting a persisted workspace override before resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceOverridePrep {
    /// Path is a directory — register it with the session manager when possible.
    Register(PathBuf),
    /// Path is missing or not a directory — keep the preference (path string), use
    /// the default workspace until it returns.
    KeepUnavailable(String),
    /// Session has no override.
    None,
}

/// Decide how to treat `workspace_override` for a resumed session.
pub(crate) fn prepare_workspace_override(
    workspace_override: Option<&str>,
) -> WorkspaceOverridePrep {
    let Some(raw) = workspace_override else {
        return WorkspaceOverridePrep::None;
    };
    let path = PathBuf::from(raw);
    if path.is_dir() {
        WorkspaceOverridePrep::Register(path)
    } else {
        WorkspaceOverridePrep::KeepUnavailable(raw.to_string())
    }
}

/// Load session metadata for resume (repository lookup only).
pub(crate) async fn load_session_for_resume(
    session_repo: &Arc<dyn SessionRepository>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))
}

/// Apply a resume into the in-memory active map.
///
/// When the session is already loaded, metadata is replaced and the stable
/// prompt cache is cleared so the next LLM call rebuilds it. Transient fields
/// (running state, messages, cancellation) stay untouched.
pub(crate) async fn activate_resumed_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session: &SessionMetadata,
    compact_context_record: Option<CompactContextRecord>,
) {
    let session_id = session.id.as_str();
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Invalidate cached stable prompt — metadata (assistant binding, name) may have
        // changed between sessions, so it must be rebuilt on the next LLM call.
        *existing_session.cached_stable_prompt.write().await = None;
        crate::agent::lifecycle::set_compact_context_if_loaded(
            existing_session,
            compact_context_record,
        )
        .await;
    } else {
        log::info!(
            "Session {} not in memory, initializing new active session state",
            session_id
        );
        // Resume historically initializes an empty registry (caller arg unused).
        active.insert(
            session_id.to_string(),
            AgentSession::new(
                session.clone(),
                Arc::new(ContextRegistry::new()),
                compact_context_record,
            ),
        );
    }
}

/// Resume an existing session by loading it into active sessions
#[allow(dead_code)]
pub async fn resume_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    _context_registry: Arc<ContextRegistry>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    let session = load_session_for_resume(session_repo, session_id).await?;

    match prepare_workspace_override(session.workspace_override.as_deref()) {
        WorkspaceOverridePrep::Register(path) => {
            if let Ok(session_manager) = crate::session::get_session_manager() {
                if let Err(e) = session_manager
                    .register_session_override(session_id, path)
                    .await
                {
                    log::warn!(
                        "Failed to pre-register workspace override for resumed session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        }
        WorkspaceOverridePrep::KeepUnavailable(workspace_override) => {
            // Keep the preference — transient path unavailability must not wipe it.
            log::warn!(
                "Persisted workspace override '{}' for session {} is unavailable or not a directory; \
                 keeping the preference and falling back to the default workspace until it returns.",
                workspace_override,
                session_id
            );
        }
        WorkspaceOverridePrep::None => {}
    }

    if session.workspace_isolation
        == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
    {
        if session.status == SessionStatus::Provisioning {
            crate::services::docker_provisioning::ensure_provisioning_task_for_session(
                &crate::services::docker_provisioning::DockerProvisioningDeps {
                    session_repo: Arc::clone(session_repo),
                    active_sessions: Arc::clone(active_sessions),
                    proxy_manager: Arc::clone(proxy_manager),
                    app_handle: Some(app_handle.clone()),
                },
                &session,
            )
            .await?;
        } else if let Err(error) =
            crate::services::WorkspaceRuntimeManager::ensure_runtime_cached(&session).await
        {
            // Store this failure: an older reason may name a different cause.
            let reason =
                crate::services::docker_provisioning::failure_reason(&session, &error.to_string());
            let dispatcher = TauriEventDispatcher::new(app_handle);
            if let Err(record_error) = record_session_error_with_dispatcher(
                session_repo,
                active_sessions,
                &dispatcher,
                session_id,
                &reason,
            )
            .await
            {
                log::warn!(
                    "Failed to record Docker runtime failure for session {}: {}",
                    session_id,
                    record_error
                );
            }
            return Err(reason);
        }
    }

    // Deserialize agent config (live assistant settings + session lineage)
    // SSOT: MCP activation is session → assistant → mcpServerIds.
    let (tool_ids, mcp_server_ids) = crate::agent::resolve_session_mcp_bindings(&session).await?;

    // Load compact context if exists (SP17)
    let compact_context_record =
        crate::agent::lifecycle::load_compact_context_record(session_id, "session resume").await?;

    activate_resumed_session(active_sessions, &session, compact_context_record).await;

    // MCP proxy creation (incl. HTTP startup) must not block session UI.
    // Workflow soft-waits via ensure_proxy_ready; discovery surfaces via runtime-state events.
    crate::agent::lifecycle::spawn_create_proxy_for_session(
        Arc::clone(proxy_manager),
        Some(app_handle.clone()),
        session_id.to_string(),
        tool_ids,
        mcp_server_ids,
    )
    .await;

    log::info!("Resumed agent session: {}", session_id);

    crate::services::hooks::spawn_session_start(
        session_id,
        crate::services::hooks::SessionStartSource::Resume,
    );

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::InMemorySessionRepository;
    use tempfile::TempDir;

    fn host_session(id: &str, workspace_override: Option<String>) -> SessionMetadata {
        SessionMetadata {
            workspace_override,
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            name: Some(format!("resume-{id}")),
            ..SessionMetadata::test_fixture(id)
        }
    }

    #[test]
    fn prepare_workspace_override_registers_an_existing_directory() {
        let dir = TempDir::new().expect("tempdir");
        let prep = prepare_workspace_override(Some(dir.path().to_str().expect("utf8")));
        assert_eq!(
            prep,
            WorkspaceOverridePrep::Register(dir.path().to_path_buf())
        );
    }

    #[test]
    fn prepare_workspace_override_keeps_unavailable_paths() {
        let dir = TempDir::new().expect("tempdir");
        let missing = dir.path().join("gone");
        let missing_str = missing.to_str().expect("utf8");
        assert_eq!(
            prepare_workspace_override(Some(missing_str)),
            WorkspaceOverridePrep::KeepUnavailable(missing_str.to_string())
        );

        let file = dir.path().join("file.txt");
        std::fs::write(&file, b"x").expect("write");
        let file_str = file.to_str().expect("utf8");
        assert_eq!(
            prepare_workspace_override(Some(file_str)),
            WorkspaceOverridePrep::KeepUnavailable(file_str.to_string())
        );

        assert_eq!(
            prepare_workspace_override(None),
            WorkspaceOverridePrep::None
        );
    }

    #[tokio::test]
    async fn load_session_for_resume_returns_not_found_for_a_missing_session() {
        let repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let err = load_session_for_resume(&repo, "does-not-exist")
            .await
            .expect_err("missing session");
        assert!(err.contains("Session not found: does-not-exist"), "{err}");
    }

    #[tokio::test]
    async fn load_session_for_resume_returns_stored_metadata() {
        let repo = Arc::new(InMemorySessionRepository::new());
        let stored = host_session("resume-me", Some("/tmp/kept-override".to_string()));
        repo.upsert_session(&stored).await.expect("upsert");
        let repo: Arc<dyn SessionRepository> = repo;
        let loaded = load_session_for_resume(&repo, "resume-me")
            .await
            .expect("found");
        assert_eq!(loaded.id, "resume-me");
        assert_eq!(
            loaded.workspace_override.as_deref(),
            Some("/tmp/kept-override")
        );
    }

    #[tokio::test]
    async fn activate_resumed_session_updates_metadata_and_clears_stable_prompt_cache() {
        let session_id = "already-active";
        let stored = host_session(session_id, None);
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));

        let active = AgentSession::new(
            SessionMetadata {
                name: Some("stale-name".to_string()),
                ..host_session(session_id, None)
            },
            Arc::new(ContextRegistry::new()),
            None,
        );
        *active.cached_stable_prompt.write().await = Some("stale-stable-prompt".to_string());
        active_sessions
            .write()
            .await
            .insert(session_id.to_string(), active);

        activate_resumed_session(&active_sessions, &stored, None).await;

        let guard = active_sessions.read().await;
        let loaded = guard.get(session_id).expect("still active");
        assert_eq!(
            loaded.metadata.name.as_deref(),
            Some("resume-already-active")
        );
        assert!(
            loaded.cached_stable_prompt.read().await.is_none(),
            "resume must invalidate the cached stable prompt"
        );
    }

    #[tokio::test]
    async fn activate_resumed_session_inserts_when_not_already_loaded() {
        let session_id = "cold-resume";
        let stored = host_session(session_id, None);
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));

        activate_resumed_session(&active_sessions, &stored, None).await;

        let guard = active_sessions.read().await;
        assert!(guard.contains_key(session_id));
        assert_eq!(
            guard
                .get(session_id)
                .expect("loaded")
                .metadata
                .name
                .as_deref(),
            Some("resume-cold-resume")
        );
    }
}

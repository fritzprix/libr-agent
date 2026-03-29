use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{CompactContextRepository, SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Parameters for session creation
pub struct CreateSessionParams {
    pub session_repo: Arc<dyn SessionRepository>,
    pub active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pub proxy_manager: Arc<MCPServiceProxyManager>,
    pub app_handle: AppHandle,
    pub context_registry: Arc<ContextRegistry>,
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
}

/// Create or update a session in the database
pub async fn create_session(params: CreateSessionParams) -> Result<SessionMetadata, String> {
    let CreateSessionParams {
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        context_registry,
        session_id,
        name,
        model,
        provider,
        mut agent_config,
    } = params;

    let now = chrono::Utc::now().timestamp_millis();

    // Validate agent config
    agent_config.validate()?;

    // Normalize lineage defaults so root sessions are always groupable in UI.
    if agent_config.lineage_id.is_none() {
        let normalized_lineage_id = if let Some(parent_id) = &agent_config.parent_session_id {
            parent_id.clone()
        } else {
            session_id.clone()
        };
        agent_config.lineage_id = Some(normalized_lineage_id);
    }

    if agent_config.depth.is_none() {
        agent_config.depth = Some(if agent_config.parent_session_id.is_some() {
            1
        } else {
            0
        });
    }

    // Serialize config for storage
    let config_json = agent_config.to_json()?;

    // Resolve mandatory model/provider
    let (resolved_model, resolved_provider) = if let (Some(m), Some(p)) = (model, provider) {
        (m, p)
    } else {
        // Fallback to global settings
        let settings_repo = crate::state::get_settings_repository();
        match settings_repo.get("preferredModel").await {
            Ok(Some(setting)) => {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&setting.value) {
                    let m = val["model"].as_str().unwrap_or("gpt-4").to_string();
                    let p = val["provider"].as_str().unwrap_or("openai").to_string();
                    (m, p)
                } else {
                    ("gpt-4".to_string(), "openai".to_string())
                }
            }
            _ => ("gpt-4".to_string(), "openai".to_string()),
        }
    };

    // Resolve workspace override: check DB first so the pool is correct before any lazy loading.
    // For existing sessions, the override may already be in the DB (set via set_override which
    // now persists to DB). For new sessions, DB returns None and WorkspaceService returns None.
    let workspace_override = if let Ok(Some(existing_session)) =
        session_repo.get_session(&session_id).await
    {
        if let Some(db_override) = existing_session.workspace_override {
            let path = std::path::PathBuf::from(&db_override);
            // Validate that the persisted path still exists and is accessible.
            // If the user has since deleted or moved the directory, fall back to the
            // default workspace and clear the stale override from the DB.
            if path.is_dir() {
                // Pre-register in pool so WorkspaceService and tools see the correct directory
                if let Ok(session_manager) = crate::session::get_session_manager() {
                    if let Err(e) = session_manager
                        .register_session_override(&session_id, path)
                        .await
                    {
                        log::warn!(
                            "Failed to pre-register workspace override for session {}: {}",
                            session_id,
                            e
                        );
                    }
                }
                Some(db_override)
            } else {
                log::warn!(
                        "Persisted workspace override '{}' for session {} no longer exists or is not a directory; \
                         clearing it and falling back to default workspace.",
                        db_override,
                        session_id
                    );
                // Best-effort clear — ignore errors since the session is still usable
                let _ = session_repo
                    .update_workspace_override(&session_id, None)
                    .await;
                None
            }
        } else {
            None
        }
    } else {
        // No existing DB record — resolve from pool (handles the case where override was
        // registered before the session was first persisted)
        crate::services::WorkspaceService::get_override(&session_id)
            .await
            .unwrap_or(None)
    };

    let session = SessionMetadata {
        id: session_id.clone(),
        name,
        status: SessionStatus::Idle,
        model: resolved_model,
        provider: resolved_provider,
        agent_config: Some(config_json),
        parent_session_id: agent_config.parent_session_id.clone(),
        lineage_id: agent_config.lineage_id.clone(),
        depth: agent_config.depth,
        max_depth: agent_config.max_depth,
        max_fanout: agent_config.max_fanout,
        is_bookmarked: false,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        yolo_mode: false,
        workspace_override,
    };

    // Persist to database using injected repository
    session_repo
        .upsert_session(&session)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    // Extract builtin tool IDs from agent config
    // Note: tools.rs already exists in src-tauri/src/agent/tools.rs
    let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);

    // Extract external MCP server IDs from agent config
    let mcp_server_ids = agent_config.mcp_server_ids.clone();

    // Create proxy for this session
    proxy_manager
        .create_proxy(
            session_id.clone(),
            tool_ids,
            mcp_server_ids,
            Some(app_handle.clone()),
        )
        .await?;

    log::info!(
        "Created MCP proxy for session: {} with builtin tools",
        session_id
    );

    // Load compact context if exists (SP17)
    let compact_context_record = {
        let repo = crate::state::get_compact_context_repository();
        repo.get_by_session_id(&session_id)
            .await
            .map_err(|e| format!("Failed to get compact context: {}", e))?
    };

    // Add to active sessions with cancellation token and empty cache
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(&session_id) {
        log::info!(
            "Session {} already active during creation/update, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Update compact context if it was loaded
        if let Some(record) = compact_context_record {
            let mut compact = existing_session.compact_context.write().await;
            *compact = Some(record);
        }
    } else {
        log::info!("Initializing new active state for session: {}", session_id);
        active.insert(
            session_id.clone(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                cancellation_token: CancellationToken::new(),
                yolo_mode: Arc::new(AtomicBool::new(session.yolo_mode)),
                cancel_pending: Arc::new(AtomicBool::new(false)),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
                thinking_only_count: Arc::new(RwLock::new(0)),
                pending_events: Arc::new(RwLock::new(
                    crate::agent::state::PendingEventManager::new(),
                )),
                pending_approvals: Arc::new(RwLock::new(std::collections::HashMap::new())),
                context_registry,
                compact_context: Arc::new(RwLock::new(compact_context_record)),
                compact_in_flight: Arc::new(AtomicBool::new(false)),
                last_compacted_tail_id: Arc::new(RwLock::new(None)),
                awaiting_compact_completion: Arc::new(AtomicBool::new(false)),
                compact_started_at_ms: Arc::new(RwLock::new(None)),
                expected_response_id: Arc::new(RwLock::new(None)),
                cached_stable_prompt: Arc::new(RwLock::new(None)),
            },
        );
    }

    log::info!("Created agent session: {}", session_id);

    // Emit resource updated event for frontend cache revalidation
    crate::agent::events::emit_resource_updated("session", "create", Some(session_id.clone()));

    Ok(session)
}

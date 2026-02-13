use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
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
        agent_config,
    } = params;

    let now = chrono::Utc::now().timestamp_millis();

    // Validate agent config
    agent_config.validate()?;

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

    let session = SessionMetadata {
        id: session_id.clone(),
        name,
        status: SessionStatus::Idle,
        model: resolved_model,
        provider: resolved_provider,
        agent_config: Some(config_json),
        created_at: now,
        updated_at: now,
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

    // Add to active sessions with cancellation token and empty cache
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(&session_id) {
        log::info!(
            "Session {} already active during creation/update, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
    } else {
        log::info!("Initializing new active state for session: {}", session_id);
        active.insert(
            session_id.clone(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                cancellation_token: CancellationToken::new(),
                cancel_pending: Arc::new(AtomicBool::new(false)),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
                thinking_only_count: Arc::new(RwLock::new(0)),
                pending_events: Arc::new(RwLock::new(
                    crate::agent::state::PendingEventManager::new(),
                )),
                context_registry,
            },
        );
    }

    log::info!("Created agent session: {}", session_id);

    // Emit resource updated event for frontend cache revalidation
    crate::agent::events::emit_resource_updated("session", "create", Some(session_id.clone()));

    Ok(session)
}

/// Resume an existing session by loading it into active sessions
#[allow(dead_code)]
pub async fn resume_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    context_registry: Arc<ContextRegistry>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    // Get session metadata from database using injected repository
    let session = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Deserialize agent config
    let agent_config = if let Some(config_json) = &session.agent_config {
        crate::agent::AgentConfig::from_json(config_json)?
    } else {
        return Err(format!("Session {} has no agent config", session_id));
    };

    // Extract builtin tool IDs from agent config
    let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);
    let mcp_server_ids = agent_config.mcp_server_ids.clone();

    // Create proxy for this session
    proxy_manager
        .create_proxy(
            session_id.to_string(),
            tool_ids,
            mcp_server_ids,
            Some(app_handle.clone()),
        )
        .await?;

    log::info!(
        "Created MCP proxy for resumed session: {} with builtin tools",
        session_id
    );

    // Add to active sessions with cancellation token and empty cache
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Transient states (pending_execution, messages, cancellation_token, etc.) are preserved
    } else {
        log::info!(
            "Session {} not in memory, initializing new active session state",
            session_id
        );
        active.insert(
            session_id.to_string(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                cancellation_token: CancellationToken::new(),
                cancel_pending: Arc::new(AtomicBool::new(false)),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
                thinking_only_count: Arc::new(RwLock::new(0)),
                pending_events: Arc::new(RwLock::new(
                    crate::agent::state::PendingEventManager::new(),
                )),
                context_registry,
            },
        );
    }

    log::info!("Resumed agent session: {}", session_id);
    Ok(session)
}

/// Update agent configuration for an existing session
pub async fn update_session_config(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _app_handle: &AppHandle,
    session_id: &str,
    model: Option<String>,
    provider: Option<String>,
    agent_config: crate::agent::AgentConfig,
) -> Result<(), String> {
    // 1. Validate new config
    agent_config.validate()?;

    // 2. Serialize config
    let config_json = agent_config.to_json()?;

    // 3. Update in database using injected repository
    session_repo
        .update_session_config(
            session_id,
            model.clone(),
            provider.clone(),
            Some(config_json.clone()),
        )
        .await
        .map_err(|e| format!("Failed to update session config: {}", e))?;

    // 4. Update active session in memory
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        if let Some(m) = model {
            session.metadata.model = m;
        }
        if let Some(p) = provider {
            session.metadata.provider = p;
        }
        session.metadata.agent_config = Some(config_json);
        session.metadata.updated_at = chrono::Utc::now().timestamp_millis();
    }

    log::info!("Updated agent config for session: {}", session_id);

    // 5. Emit event to notify frontend of config change
    // We reuse StatusChanged or create a new event.
    // Ideally we should have a `ConfigChanged` event, but `StatusChanged` might force a refresh if the frontend re-fetches.
    // However, to be explicit, let's just log for now. The frontend will update its local state optimistically or re-fetch.
    // Or we can emit `AgentEvent::StatusChanged` if we want to force some side-effects, but that's hacky.

    // Let's rely on the command response for the immediate update,
    // and if we need broadcast, we'd add `ConfigChanged` to `AgentEvent`.
    // For now, simple DB update is enough as the frontend driving this change will know it happened.

    Ok(())
}

/// Update session status in database and emit event
pub async fn update_session_status(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    status: SessionStatus,
) -> Result<(), String> {
    session_repo
        .update_status(session_id, status.clone())
        .await
        .map_err(|e| format!("Failed to update session status: {}", e))?;

    // Update in-memory state
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        session.metadata.status = status.clone();
    }

    // Emit status changed event
    let event = crate::agent::events::AgentEvent::StatusChanged {
        session_id: session_id.to_string(),
        status,
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Recover sessions stuck in BUSY state after app crash/restart
pub async fn recover_sessions(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    context_registry: Arc<ContextRegistry>,
) -> Result<(), String> {
    log::info!("Starting session recovery process...");

    let all_sessions = session_repo
        .get_all_sessions()
        .await
        .map_err(|e| format!("Failed to query sessions for recovery: {}", e))?;

    let mut recovered_count = 0;

    for session in all_sessions {
        // Only recover sessions that were BUSY (actively running)
        if matches!(session.status, SessionStatus::Busy) {
            log::warn!(
                "Recovering session '{}' from BUSY state (possible crash)",
                session.id
            );

            // Reset to PAUSED (user can manually resume)
            // Call local update_session_status helper
            update_session_status(
                session_repo,
                active_sessions,
                app_handle,
                &session.id,
                SessionStatus::Paused,
            )
            .await?;

            // Initialize session in active_sessions map with fresh state
            let mut active = active_sessions.write().await;
            if let Some(existing_session) = active.get_mut(&session.id) {
                log::info!(
                    "Session {} already active during recovery, updating metadata only",
                    session.id
                );
                existing_session.metadata = session.clone();
            } else {
                log::info!(
                    "Initializing new active state for recovered session: {}",
                    session.id
                );
                active.insert(
                    session.id.clone(),
                    AgentSession {
                        metadata: session.clone(),
                        is_running: false,
                        cancellation_token: CancellationToken::new(),
                        cancel_pending: Arc::new(AtomicBool::new(false)),
                        pending_execution: None,
                        messages: Arc::new(RwLock::new(Vec::new())),
                        cache_initialized: Arc::new(AtomicBool::new(false)),
                        last_synced_at: Arc::new(RwLock::new(None)),
                        thinking_only_count: Arc::new(RwLock::new(0)),
                        pending_events: Arc::new(RwLock::new(
                            crate::agent::state::PendingEventManager::new(),
                        )),
                        context_registry: context_registry.clone(),
                    },
                );
            }
            drop(active); // Release lock early

            recovered_count += 1;
        }
    }

    if recovered_count > 0 {
        log::info!(
            "Session recovery complete: {} session(s) recovered",
            recovered_count
        );
    } else {
        log::info!("Session recovery complete: No sessions to recover");
    }

    Ok(())
}

/// Get session metadata
pub async fn get_session(
    session_repo: &Arc<dyn SessionRepository>,
    session_id: &str,
) -> Result<Option<SessionMetadata>, String> {
    session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))
}

/// Get all sessions from database
pub async fn get_all_sessions(
    session_repo: &Arc<dyn SessionRepository>,
) -> Result<Vec<SessionMetadata>, String> {
    session_repo
        .get_all_sessions()
        .await
        .map_err(|e| format!("Failed to get all sessions: {}", e))
}

/// Load messages from DB into in-memory cache (called once per session)
pub async fn init_session_with_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let message_repo = crate::state::get_message_repository();

    // Load last 1000 messages from DB (one-time operation)
    let page = message_repo
        .get_page(session_id, 1, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for session {}: {}", session_id, e))?;

    let loaded_count = page.items.len();

    // Populate in-memory cache
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        *messages = page.items; // Replace with DB data

        let mut synced_at = session.last_synced_at.write().await;
        *synced_at = Some(SystemTime::now());

        session.cache_initialized.store(true, Ordering::Release);

        log::info!(
            "Initialized session cache: session={}, messages_loaded={}",
            session_id,
            loaded_count
        );
    } else {
        return Err(format!("Session not found: {}", session_id));
    }

    Ok(())
}

/// Ensure cache is initialized before workflow starts (lazy initialization)
pub async fn ensure_cache_initialized(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        if !session.cache_initialized.load(Ordering::Acquire) {
            drop(sessions); // Release read lock before calling init
            init_session_with_messages(active_sessions, session_id).await?;
        }
    }
    Ok(())
}

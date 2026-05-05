use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::management::update_session_status_with_dispatcher;

/// Close orphaned tool calls for a recovered session by injecting synthetic error responses.
/// Prevents the UI from showing stuck running spinners after a crash recovery.
async fn close_orphaned_tool_calls(session_id: &str) -> Result<(), String> {
    let message_repo = crate::state::get_message_repository();

    // Load messages for the session (up to MAX_CACHED_MESSAGES)
    let page = message_repo
        .get_page(session_id, 1, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for tombstone check: {}", e))?;

    let messages = page.items;

    // Collect all resolved tool_call_ids (role="tool" messages with a tool_call_id)
    let resolved_ids: HashSet<String> = messages
        .iter()
        .filter(|m| m.role == "tool")
        .filter_map(|m| m.tool_call_id.clone())
        .collect();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut tombstones: Vec<crate::models::chat::Message> = Vec::new();

    // Find assistant messages with unresolved tool calls
    for msg in &messages {
        if msg.role != "assistant" {
            continue;
        }
        let Some(tool_calls) = &msg.tool_calls else {
            continue;
        };
        for tc in tool_calls {
            if resolved_ids.contains(&tc.id) {
                continue;
            }
            // Inject a synthetic error result tombstone so the UI can unblock
            tombstones.push(crate::models::chat::Message {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
                role: "tool".to_string(),
                content: vec![crate::mcp::types::MCPContent::Text {
                    text: "[system] Tool call did not complete (session recovered after crash)."
                        .to_string(),
                    is_error: Some(true),
                }],
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
                is_streaming: None,
                thinking: None,
                thinking_signature: None,
                assistant_id: None,
                usage: None,
                attachments: None,
                tool_use: None,
                created_at: now,
                updated_at: now,
                source: Some("recovery".to_string()),
                error: None,
                metadata: None,
            });
        }
    }

    if tombstones.is_empty() {
        return Ok(());
    }

    log::info!(
        "Inserting {} tombstone(s) for orphaned tool calls in session '{}'",
        tombstones.len(),
        session_id
    );

    message_repo
        .insert_many(tombstones)
        .await
        .map_err(|e| format!("Failed to insert tombstone messages: {}", e))?;

    Ok(())
}

fn build_recovered_session(
    session: &crate::repositories::SessionMetadata,
    context_registry: Arc<ContextRegistry>,
) -> AgentSession {
    AgentSession {
        metadata: session.clone(),
        is_running: false,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        yolo_mode: Arc::new(AtomicBool::new(session.yolo_mode)),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(false)),
        last_synced_at: Arc::new(RwLock::new(None)),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(crate::agent::state::PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(std::collections::HashMap::new())),
        context_registry,
        compact_context: Arc::new(RwLock::new(None)),
        compaction: crate::agent::state::CompactionRuntimeState::new(),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
    }
}

/// Recover sessions stuck in BUSY state after app crash/restart
pub async fn recover_sessions(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    context_registry: Arc<ContextRegistry>,
) -> Result<(), String> {
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    recover_sessions_with_dispatcher(session_repo, active_sessions, &dispatcher, context_registry)
        .await
}

/// Recover BUSY sessions without depending on a live Tauri handle.
/// Used by the app on startup and by integration tests with a recording dispatcher.
pub async fn recover_sessions_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
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

            let mut recovered_metadata = session.clone();
            recovered_metadata.status = SessionStatus::Paused;

            // Ensure the session exists in memory before persisting the pause transition.
            // Recovery used to call update_session_status first, which failed because the
            // crashed session was not yet present in active_sessions.
            let mut active = active_sessions.write().await;
            if let Some(existing_session) = active.get_mut(&session.id) {
                log::info!(
                    "Session {} already active during recovery, updating metadata only",
                    session.id
                );
                existing_session.metadata = recovered_metadata.clone();
                existing_session.is_running = false;
            } else {
                log::info!(
                    "Initializing new active state for recovered session: {}",
                    session.id
                );
                active.insert(
                    session.id.clone(),
                    build_recovered_session(&recovered_metadata, context_registry.clone()),
                );
            }
            drop(active); // Release lock early

            update_session_status_with_dispatcher(
                session_repo,
                active_sessions,
                dispatcher,
                &session.id,
                SessionStatus::Paused,
            )
            .await?;

            // Close any orphaned tool calls that never got a result (crash tombstones)
            if let Err(e) = close_orphaned_tool_calls(&session.id).await {
                log::warn!(
                    "Failed to close orphaned tool calls for session '{}': {}",
                    session.id,
                    e
                );
            }

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

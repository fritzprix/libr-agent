use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::models::chat::MessageSource;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::management::update_session_status_with_dispatcher;

/// Close orphaned tool calls for a recovered session by injecting synthetic error responses.
/// Prevents the UI from showing stuck running spinners after a crash recovery.
async fn close_orphaned_tool_calls(
    session_id: &str,
    dispatcher: &dyn AgentEventDispatcher,
) -> Result<(), String> {
    let message_repo = crate::state::get_message_repository();

    // Only the most recent causal window matters for unresolved tool-call recovery.
    let recent_slice = message_repo
        .get_recent_slice(session_id, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for tombstone check: {}", e))?;

    let messages = recent_slice.items;

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

    // Matches create_error_tool_result / frontend useMessageGrouping toolError contract.
    let tool_error_metadata = Some(serde_json::json!({ "toolError": true }));

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
                }],
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
                is_streaming: None,
                thinking: None,
                thinking_signature: None,
                assistant_id: None,
                usage: None,
                prompt_tokens: None,
                attachments: None,
                tool_use: None,
                created_at: now,
                updated_at: now,
                source: Some(MessageSource::Recovery),
                error: None,
                metadata: tool_error_metadata.clone(),
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
        .insert_many(tombstones.clone())
        .await
        .map_err(|e| format!("Failed to insert tombstone messages: {}", e))?;

    for tombstone in &tombstones {
        dispatcher
            .emit_agent_event(AgentEvent::MessageAdded {
                session_id: session_id.to_string(),
                message: Box::new(tombstone.clone()),
            })
            .map_err(|e| format!("Failed to emit MessageAdded for recovery tombstone: {}", e))?;
    }

    Ok(())
}

fn build_recovered_session(
    session: &crate::repositories::SessionMetadata,
    context_registry: Arc<ContextRegistry>,
) -> AgentSession {
    AgentSession::new(session.clone(), context_registry, None)
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
        // Only recover sessions that were BUSY (actively running) or QUEUED (waiting for concurrency slot)
        if matches!(session.status, SessionStatus::Busy | SessionStatus::Queued) {
            let target_status = match session.status {
                SessionStatus::Busy => SessionStatus::Paused,
                _ => SessionStatus::Idle,
            };

            log::warn!(
                "Recovering session '{}' from {:?} state (possible crash)",
                session.id,
                session.status
            );

            let mut recovered_metadata = session.clone();
            recovered_metadata.status = target_status.clone();

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
                target_status.clone(),
            )
            .await?;

            if matches!(session.status, SessionStatus::Busy) {
                // Close any orphaned tool calls that never got a result (crash tombstones)
                if let Err(e) = close_orphaned_tool_calls(&session.id, dispatcher).await {
                    log::warn!(
                        "Failed to close orphaned tool calls for session '{}': {}",
                        session.id,
                        e
                    );
                }
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

use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Result of truncating the last completed assistant turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncateLastTurnPlan {
    pub anchor_user_message_id: String,
    pub deleted_message_ids: Vec<String>,
}

/// Find the last external user message and collect all following transcript rows
/// (assistant + tool) that form the completed turn to discard.
pub fn plan_truncate_last_turn(messages: &[Message]) -> Result<TruncateLastTurnPlan, String> {
    let anchor_idx = messages
        .iter()
        .rposition(|message| {
            message.role == "user" && !message.is_internal_synthetic_user_message()
        })
        .ok_or_else(|| {
            "Cannot regenerate: no user message found to re-submit from".to_string()
        })?;

    let tail = &messages[anchor_idx + 1..];
    if tail.is_empty() {
        return Err(
            "Cannot regenerate: no assistant turn after the last user message".to_string(),
        );
    }

    if !tail
        .iter()
        .any(|message| message.role == "assistant" || message.role == "tool")
    {
        return Err(
            "Cannot regenerate: last turn has no assistant or tool messages to discard"
                .to_string(),
        );
    }

    for message in tail {
        if message.role == "user" && !message.is_internal_synthetic_user_message() {
            return Err(
                "Cannot regenerate: unexpected user message after the last user turn".to_string(),
            );
        }
    }

    Ok(TruncateLastTurnPlan {
        anchor_user_message_id: messages[anchor_idx].id.clone(),
        deleted_message_ids: tail.iter().map(|message| message.id.clone()).collect(),
    })
}

/// Discard the last completed assistant turn (idle only) and re-trigger LLM completion.
pub async fn regenerate_last_turn(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<TruncateLastTurnPlan, String> {
    {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session {} not found in active sessions", session_id))?;

        if session.metadata.status != SessionStatus::Idle {
            return Err(format!(
                "Cannot regenerate last turn for session {}: status is {:?} (must be Idle)",
                session_id, session.metadata.status
            ));
        }

        if session.compaction.snapshot().await.is_in_flight() {
            return Err(format!(
                "Cannot regenerate last turn for session {} while compaction is in flight",
                session_id
            ));
        }

        if !session.pending_approvals.read().await.is_empty() {
            return Err(format!(
                "Cannot regenerate last turn for session {}: pending tool approvals remain",
                session_id
            ));
        }
    }

    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    let plan = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session {} not found in active sessions", session_id))?;
        let messages = session.messages.read().await;
        let plan = plan_truncate_last_turn(&messages)?;

        let compact = session.compact_context.read().await;
        if let Some(record) = compact.as_ref() {
            if plan
                .deleted_message_ids
                .iter()
                .any(|id| id == &record.to_id)
            {
                return Err(
                    "Cannot regenerate: last turn overlaps the compact context boundary"
                        .to_string(),
                );
            }
        }
        plan
    };

    // Re-check Idle under write lock before mutating history (status race).
    {
        let mut active = active_sessions.write().await;
        let session = active
            .get_mut(&session_id)
            .ok_or_else(|| format!("Session {} not found in active sessions", session_id))?;

        if session.metadata.status != SessionStatus::Idle {
            return Err(format!(
                "Cannot regenerate last turn for session {}: status changed to {:?}",
                session_id, session.metadata.status
            ));
        }

        let mut messages = session.messages.write().await;
        let delete_set: std::collections::HashSet<&str> = plan
            .deleted_message_ids
            .iter()
            .map(String::as_str)
            .collect();
        messages.retain(|message| !delete_set.contains(message.id.as_str()));
    }

    let message_repo = crate::state::get_message_repository();
    for message_id in &plan.deleted_message_ids {
        message_repo
            .delete_by_id(message_id)
            .await
            .map_err(|e| format!("Failed to delete message {}: {}", message_id, e))?;
    }

    let remove_event = crate::agent::events::AgentEvent::MessagesRemoved {
        session_id: session_id.clone(),
        message_ids: plan.deleted_message_ids.clone(),
    };
    if let Err(error) = crate::agent::tauri_events::emit_agent_event(app_handle, remove_event) {
        log::warn!(
            "Failed to emit MessagesRemoved for session {}: {}",
            session_id,
            error
        );
    }

    log::info!(
        "Regenerating last turn for session {}: truncated {} message(s) after user {}",
        session_id,
        plan.deleted_message_ids.len(),
        plan.anchor_user_message_id
    );

    crate::agent::workflow::resume_workflow(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await?;

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chat::{Message, MessageSource};

    fn msg(id: &str, role: &str) -> Message {
        Message {
            id: id.to_string(),
            session_id: "s1".to_string(),
            role: role.to_string(),
            content: vec![],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 1,
            updated_at: 1,
            source: None,
            error: None,
            metadata: None,
        }
    }

    fn synthetic_user(id: &str) -> Message {
        let mut message = msg(id, "user");
        message.source = Some(MessageSource::SessionContext);
        message
    }

    #[test]
    fn plans_simple_user_assistant_tail() {
        let messages = vec![msg("u1", "user"), msg("a1", "assistant")];
        let plan = plan_truncate_last_turn(&messages).expect("plan");
        assert_eq!(plan.anchor_user_message_id, "u1");
        assert_eq!(plan.deleted_message_ids, vec!["a1".to_string()]);
    }

    #[test]
    fn plans_tool_loop_tail() {
        let messages = vec![
            msg("u0", "user"),
            msg("a0", "assistant"),
            msg("u1", "user"),
            msg("a1", "assistant"),
            msg("t1", "tool"),
            msg("a2", "assistant"),
        ];
        let plan = plan_truncate_last_turn(&messages).expect("plan");
        assert_eq!(plan.anchor_user_message_id, "u1");
        assert_eq!(
            plan.deleted_message_ids,
            vec![
                "a1".to_string(),
                "t1".to_string(),
                "a2".to_string()
            ]
        );
    }

    #[test]
    fn skips_synthetic_user_as_anchor() {
        let messages = vec![
            msg("u1", "user"),
            msg("a1", "assistant"),
            synthetic_user("syn"),
            msg("a2", "assistant"),
        ];
        let plan = plan_truncate_last_turn(&messages).expect("plan");
        assert_eq!(plan.anchor_user_message_id, "u1");
        assert_eq!(
            plan.deleted_message_ids,
            vec![
                "a1".to_string(),
                "syn".to_string(),
                "a2".to_string()
            ]
        );
    }

    #[test]
    fn rejects_when_no_tail() {
        let messages = vec![msg("u1", "user")];
        let err = plan_truncate_last_turn(&messages).expect_err("must fail");
        assert!(err.contains("no assistant turn"));
    }

    #[test]
    fn rejects_when_no_user() {
        let messages = vec![msg("a1", "assistant")];
        let err = plan_truncate_last_turn(&messages).expect_err("must fail");
        assert!(err.contains("no user message"));
    }
}

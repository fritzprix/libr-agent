use crate::agent::AgentSessionManager;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use std::sync::Arc;
use uuid::Uuid;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::{ErrorResponse, GetMessagesQuery, SendMessageRequest, SendMessageResponse};

pub async fn get_messages(
    id: String,
    query: GetMessagesQuery,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    // Validate session existence before fetching messages.
    // Without this check the message repo silently returns an empty list
    // for any unknown session ID, masking bugs.
    match manager.get_session(&id).await {
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Session not found: {}", id),
                }),
                StatusCode::NOT_FOUND,
            ))
        }
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to validate session: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
        Ok(Some(_)) => {} // Session exists, proceed
    }

    let repo = crate::state::get_message_repository();
    use crate::repositories::message_repository::MessageRepository;
    let limit = query.limit.unwrap_or(50);

    match repo.get_messages_by_session(&id, limit).await {
        Ok(messages) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "messages": messages })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch messages: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn send_message(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: SendMessageRequest,
) -> Result<impl Reply, Rejection> {
    // 1. Check session existence and status
    let session_opt = match manager.get_session(&id).await {
        Ok(s) => s,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error: e }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    };

    if session_opt.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        ));
    }

    let session = session_opt.unwrap();
    let is_busy = matches!(session.status, crate::repositories::SessionStatus::Busy);

    // NOTE: Session.agent_config may contain partial/legacy JSON in some code paths (e.g., tests).
    // Avoid strict AgentConfig deserialization here; extract the assistant ID from common fields.
    let assistant_id = session.agent_config.as_ref().and_then(|config_str| {
        let config: serde_json::Value = match serde_json::from_str(config_str) {
            Ok(v) => v,
            Err(e) => {
                log::warn!(
                    "Invalid session.agent_config JSON for session {} (assistant_id will be None): {}",
                    id,
                    e
                );
                return None;
            }
        };

        let assistant_id_value = config
            .get("assistant_id")
            .or_else(|| config.get("assistantId"))
            .or_else(|| config.get("id"));

        match assistant_id_value {
            Some(v) => match v.as_str() {
                Some(s) => Some(s.to_string()),
                None => {
                    log::warn!(
                        "session.agent_config assistant id field is not a string for session {} (assistant_id will be None)",
                        id
                    );
                    None
                }
            },
            None => {
                log::warn!(
                    "No assistant id field found in session.agent_config for session {} (expected one of: assistant_id, assistantId, id)",
                    id
                );
                None
            }
        }
    });

    // 2. Create Message object
    let message_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let message = Message {
        id: message_id.clone(),
        session_id: id.clone(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: body.content,
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id,
        usage: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: body.source.or_else(|| Some("api".to_string())),
        error: None,
        metadata: None,
    };

    // 3. Handle based on status
    if is_busy {
        // Queue the message
        log::info!("Session {} is busy. Queuing message {}.", id, message_id);
        if let Err(e) = manager
            .inject_messages(id.clone(), vec![message], false)
            .await
        {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to queue message: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }

        Ok(warp::reply::with_status(
            warp::reply::json(&SendMessageResponse {
                id: message_id,
                status: "queued".to_string(),
            }),
            StatusCode::OK, // 200 OK, but status indicates queued
        ))
    } else {
        // Trigger workflow
        log::info!(
            "Session {} is idle. Starting workflow with message {}.",
            id,
            message_id
        );
        if let Err(e) = manager.start_workflow(id.clone(), message).await {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to start workflow: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }

        Ok(warp::reply::with_status(
            warp::reply::json(&SendMessageResponse {
                id: message_id,
                status: "processed".to_string(),
            }),
            StatusCode::OK,
        ))
    }
}

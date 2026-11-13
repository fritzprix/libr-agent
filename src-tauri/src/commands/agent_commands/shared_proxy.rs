//! Forwarding helpers for GUI attach mode (#2701).
//!
//! When a second GUI connects to an already-running shared backend, session,
//! workflow, and channel commands route through [`SharedBackendClient`] to the
//! single backend's Session API instead of running a second local executor.

use super::contracts::{
    AgentOpenSessionResponse, AgentResponse, AgentSessionListResponse, CreateAgentSessionRequest,
    CreateAgentSessionWithMessageRequest, ListAgentSessionsRequest, PendingApprovalSnapshot,
};
use crate::agent::events::AgentEvent;
use crate::agent::state::PendingUserPromptSummary;
use crate::agent::tauri_events::emit_agent_event;
use crate::commands::messages_commands::MessageSlice;
use crate::models::chat::Message;
use crate::repositories::{SessionMetadata, SessionStatus};
use crate::server::shared_backend::SharedBackendClient;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

static SUBSCRIBED_SESSIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn subscribed_sessions() -> &'static Mutex<HashSet<String>> {
    SUBSCRIBED_SESSIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Ensure at most one background SSE forwarder is running for `session_id` in
/// this attached GUI so live status and message updates reach its WebView.
pub fn ensure_session_event_stream(client: &SharedBackendClient, session_id: &str) {
    let mut guard = match subscribed_sessions().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if !guard.insert(session_id.to_string()) {
        return;
    }
    drop(guard);

    let url = client.events_url(session_id);
    let http = client.http_client().clone();
    let sid = session_id.to_string();

    tauri::async_runtime::spawn(async move {
        let result = forward_sse_events(&http, &url, &sid).await;
        if let Err(err) = result {
            log::debug!("Shared backend SSE stream ended for {sid}: {err}");
        }
        if let Ok(mut guard) = subscribed_sessions().lock() {
            guard.remove(&sid);
        }
    });
}

async fn forward_sse_events(
    http: &reqwest::Client,
    url: &str,
    session_id: &str,
) -> Result<(), String> {
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to SSE {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("SSE {url} returned HTTP {}", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("SSE read error: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer.drain(..=pos);

            if line.is_empty() {
                if !data_lines.is_empty() {
                    let payload = data_lines.join("\n");
                    dispatch_sse_event(&event_name, &payload, session_id);
                    if event_name == "end" {
                        return Ok(());
                    }
                    event_name.clear();
                    data_lines.clear();
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("event:") {
                event_name = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.trim_start().to_string());
            }
        }
    }
    Ok(())
}

fn dispatch_sse_event(event_name: &str, payload: &str, session_id: &str) {
    let Some(app_handle) = crate::state::get_app_handle() else {
        return;
    };
    match event_name {
        "status" => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
                if let Some(status_val) = val.get("status").cloned() {
                    if let Ok(status) = serde_json::from_value::<SessionStatus>(status_val) {
                        let status_reason = val
                            .get("statusReason")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let _ = emit_agent_event(
                            app_handle,
                            AgentEvent::StatusChanged {
                                session_id: session_id.to_string(),
                                status,
                                status_reason,
                            },
                        );
                    }
                }
            }
        }
        "message" => {
            if let Ok(message) = serde_json::from_str::<Message>(payload) {
                let _ = emit_agent_event(
                    app_handle,
                    AgentEvent::MessageAdded {
                        session_id: session_id.to_string(),
                        message: Box::new(message),
                    },
                );
            }
        }
        _ => {}
    }
}

pub async fn create_session(
    client: &SharedBackendClient,
    request: CreateAgentSessionRequest,
) -> Result<SessionMetadata, String> {
    let body = serde_json::json!({
        "name": request.name,
        "assistantId": request.agent_config.id,
        "model": request.model,
        "provider": request.provider,
        "workspacePath": request.workspace_path,
        "workspaceIsolation": request.workspace_isolation,
        "dockerConfig": request.docker_config,
    });
    let created = client.create_session(&body).await?;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Shared backend create_session response missing id".to_string())?;
    let session_json = client.get_session(id).await?;
    serde_json::from_value::<SessionMetadata>(session_json)
        .map_err(|e| format!("Failed to parse session metadata from shared backend: {e}"))
}

pub async fn create_session_with_initial_message(
    client: &SharedBackendClient,
    request: CreateAgentSessionWithMessageRequest,
) -> Result<AgentResponse, String> {
    let prompt_text = request
        .message
        .content
        .iter()
        .filter_map(|c| match c {
            crate::mcp::types::MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = serde_json::json!({
        "name": request.name,
        "assistantId": request.agent_config.id,
        "model": request.model,
        "provider": request.provider,
        "workspacePath": request.workspace_path,
        "workspaceIsolation": request.workspace_isolation,
        "dockerConfig": request.docker_config,
        "request": prompt_text,
    });
    let created = client.create_session(&body).await?;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    ensure_session_event_stream(client, &id);
    Ok(AgentResponse {
        success: true,
        message: format!("Session created on shared backend: {id}"),
        data: Some(created),
    })
}

pub async fn open_session(
    client: &SharedBackendClient,
    session_id: &str,
    initial_message_limit: u64,
) -> Result<AgentOpenSessionResponse, String> {
    let session_json = client.get_session(session_id).await?;
    let session: SessionMetadata = serde_json::from_value(session_json.clone())
        .map_err(|e| format!("Failed to parse session from shared backend: {e}"))?;

    let pending_approvals: Vec<PendingApprovalSnapshot> = session_json
        .get("pendingApprovals")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let pending_user_prompt: Option<PendingUserPromptSummary> = session_json
        .get("pendingUserPrompt")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok());

    let msgs_json = client
        .get_messages(&session.id, Some(initial_message_limit))
        .await?;
    let items: Vec<Message> = msgs_json
        .get("messages")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    ensure_session_event_stream(client, &session.id);

    Ok(AgentOpenSessionResponse {
        session,
        messages: MessageSlice {
            items,
            has_more_before: false,
            oldest_cursor: None,
        },
        pending_approvals,
        pending_user_prompt,
        runtime_state: Default::default(),
    })
}

pub async fn list_sessions(
    client: &SharedBackendClient,
    request: Option<ListAgentSessionsRequest>,
) -> Result<AgentSessionListResponse, String> {
    let limit = request.and_then(|r| r.limit);
    let json = client.list_sessions(limit).await?;
    let items: Vec<SessionMetadata> = json
        .get("sessions")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(AgentSessionListResponse {
        items,
        next_cursor: None,
    })
}

pub async fn get_all_sessions(
    client: &SharedBackendClient,
) -> Result<Vec<SessionMetadata>, String> {
    let json = client.list_sessions(Some(200)).await?;
    json.get("sessions")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(|| "Shared backend list_sessions missing sessions array".to_string())
}

pub async fn get_session(
    client: &SharedBackendClient,
    session_id: &str,
) -> Result<Option<SessionMetadata>, String> {
    match client.get_session(session_id).await {
        Ok(val) => serde_json::from_value(val)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(_) => Ok(None),
    }
}

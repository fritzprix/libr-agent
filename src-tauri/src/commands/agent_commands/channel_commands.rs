use super::contracts::{
    AgentResponse, InjectChannelMessageAutoRequest, InjectChannelMessageRequest,
    RespondChannelPermissionRequest,
};
use crate::agent::AgentSessionManager;
use crate::mcp::types::{ChannelNotification, ChannelPermissionVerdict};
use tauri::{command, State};

/// Inject a channel-originated message into the session and wake the workflow when idle.
#[command]
pub async fn agent_inject_channel_message(
    manager: State<'_, AgentSessionManager>,
    request: InjectChannelMessageRequest,
) -> Result<AgentResponse, String> {
    let (message_id, triggered) = manager
        .inject_channel_notification(
            request.session_id.clone(),
            request.server_name.clone(),
            ChannelNotification {
                content: request.content,
                meta: request.meta,
            },
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected channel message for session: {} from {} ({})",
            request.session_id,
            request.server_name,
            if triggered { "processed" } else { "queued" }
        ),
        data: Some(serde_json::json!({
            "messageId": message_id,
            "status": if triggered { "processed" } else { "queued" }
        })),
    })
}

/// Inject a channel-originated message into the uniquely matching active session for the given
/// channel server and wake the workflow when idle.
#[command]
pub async fn agent_inject_channel_message_auto(
    manager: State<'_, AgentSessionManager>,
    request: InjectChannelMessageAutoRequest,
) -> Result<AgentResponse, String> {
    let (target, message_id, triggered) = manager
        .inject_channel_notification_auto(
            request.server_name.clone(),
            ChannelNotification {
                content: request.content,
                meta: request.meta,
            },
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected channel message from {} into session {} ({})",
            request.server_name,
            target.session_name,
            if triggered { "processed" } else { "queued" }
        ),
        data: Some(serde_json::json!({
            "messageId": message_id,
            "sessionId": target.session_id,
            "sessionName": target.session_name,
            "status": if triggered { "processed" } else { "queued" }
        })),
    })
}

/// Respond to a pending approval using a channel-style request_id and allow/deny behavior.
#[command]
pub async fn agent_respond_channel_permission(
    manager: State<'_, AgentSessionManager>,
    request: RespondChannelPermissionRequest,
) -> Result<AgentResponse, String> {
    let verdict = ChannelPermissionVerdict {
        request_id: request.request_id.clone(),
        behavior: request.behavior.clone(),
    };

    let approved =
        crate::agent::tool_approvals::parse_channel_permission_behavior(&verdict.behavior)?;

    let tool_call_id = manager
        .respond_channel_permission(&request.session_id, &verdict.request_id, approved)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Channel permission responded for request {} (tool_call_id: {}, approved: {})",
            verdict.request_id, tool_call_id, approved
        ),
        data: Some(serde_json::json!({
            "toolCallId": tool_call_id,
            "approved": approved,
        })),
    })
}

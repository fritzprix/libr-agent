use crate::commands::messages_commands::MessageSlice;
use crate::models::chat::Message;
use crate::repositories::{SessionListCursor, SessionListPage, SessionMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request to create a new agent session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    #[serde(default)]
    pub is_ephemeral: bool,
    pub workspace_path: Option<String>,
}

/// Request to create a new session and send the first message in one go
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionWithMessageRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    pub message: Message,
    pub workspace_path: Option<String>,
}

/// Request to send a user message to trigger workflow
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub session_id: String,
    pub message: Message,
}

/// Request to inject messages. Workflow continuation is decided by backend
/// session state. `trigger_workflow` is kept only for backward compatibility.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMessagesRequest {
    pub session_id: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub trigger_workflow: bool,
}

fn default_ui_action_params() -> serde_json::Value {
    serde_json::json!({})
}

/// Request to execute a UI-triggered Tauri action through the backend-owned message path.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteUiTauriActionRequest {
    pub session_id: String,
    pub tool_name: String,
    #[serde(default = "default_ui_action_params")]
    pub params: serde_json::Value,
}

/// Request to inject a channel-originated message into a session.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectChannelMessageRequest {
    pub session_id: String,
    pub server_name: String,
    pub content: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Request to inject a channel-originated message using automatic active-session routing.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectChannelMessageAutoRequest {
    pub server_name: String,
    pub content: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Request to respond to a pending channel permission relay approval.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespondChannelPermissionRequest {
    pub session_id: String,
    pub request_id: String,
    pub behavior: String,
}

/// Request to update agent configuration for a session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentConfigRequest {
    pub session_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
}

/// Response for agent operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOpenSessionResponse {
    pub session: SessionMetadata,
    pub messages: MessageSlice,
    #[serde(default)]
    pub pending_approvals: Vec<PendingApprovalSnapshot>,
    #[serde(default)]
    pub runtime_state: crate::agent::runtime_state::SessionRuntimeState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApprovalSnapshot {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub approval_kind: crate::agent::state::PendingApprovalKind,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_preview: Option<String>,
}

/// Tool execution result from frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub success: bool,
    pub content: String,
    pub mcp_content: Option<Vec<crate::mcp::types::MCPContent>>,
    pub structured_content: Option<serde_json::Value>,
    pub error: Option<String>,
    pub is_error: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentSessionsRequest {
    pub cursor: Option<SessionListCursorDto>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListCursorDto {
    pub updated_at: i64,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionListResponse {
    pub items: Vec<SessionMetadata>,
    pub next_cursor: Option<SessionListCursorDto>,
}

impl From<SessionListCursor> for SessionListCursorDto {
    fn from(value: SessionListCursor) -> Self {
        Self {
            updated_at: value.updated_at,
            id: value.id,
        }
    }
}

impl From<SessionListCursorDto> for SessionListCursor {
    fn from(value: SessionListCursorDto) -> Self {
        Self {
            updated_at: value.updated_at,
            id: value.id,
        }
    }
}

impl From<SessionListPage> for AgentSessionListResponse {
    fn from(value: SessionListPage) -> Self {
        Self {
            items: value.items,
            next_cursor: value.next_cursor.map(Into::into),
        }
    }
}

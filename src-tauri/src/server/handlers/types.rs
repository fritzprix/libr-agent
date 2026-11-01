use crate::agent::session_manager::PendingApprovalSummary;
pub use crate::agent::types::CreateSessionRequest;
use crate::execution_mode::ExecutionMode;
use crate::models::chat::MessageSource;
use crate::repositories::SessionMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub source: Option<MessageSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectChannelRequest {
    pub server_name: String,
    pub content: String,
    #[serde(default)]
    pub meta: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPermissionRequestBody {
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    pub behavior: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSessionRequest {
    pub execution_mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSessionResponse {
    pub id: String,
    pub execution_mode: ExecutionMode,
    pub resolved_approval_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusResponse {
    #[serde(flatten)]
    pub session: SessionMetadata,
    pub pending_approvals: Vec<PendingApprovalSummary>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub status: String, // "processed" or "queued"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRouteChannelResponse {
    pub id: String,
    pub session_id: String,
    pub session_name: String,
    pub status: String, // "processed" or "queued"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPermissionResponse {
    pub request_id: Option<String>,
    pub tool_call_id: String,
    pub approved: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildSessionsResponse {
    pub parent_session_id: String,
    pub count: usize,
    pub children: Vec<String>,
}

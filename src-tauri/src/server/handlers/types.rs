use crate::agent::session_manager::PendingApprovalSummary;
pub use crate::agent::types::CreateSessionRequest;
use crate::execution_mode::ExecutionMode;
use crate::models::chat::MessageSource;
use crate::repositories::SessionMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub build_sha: &'static str,
    pub build_dirty: bool,
    /// False while startup session recovery is still running; true once the
    /// runtime is ready to run sessions (see `server::readiness`).
    pub ready: bool,
    /// Outcome of startup session recovery: `pending`, `ok`, `failed`, or
    /// `skipped` (another live backend owns the sessions).
    pub recovery: crate::server::readiness::RecoveryState,
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
    /// The prompt still waiting for this session (#2543 requirement 7).
    /// Never carries a secret value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_user_prompt: Option<crate::agent::state::PendingUserPromptSummary>,
}

/// Query of `GET /api/sessions`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsQuery {
    pub limit: Option<u64>,
    /// With `cursor_id`: the `nextCursor` of the previous page.
    pub cursor_updated_at: Option<i64>,
    pub cursor_id: Option<String>,
    /// Case-insensitive match on session name/id or assistant name.
    pub search: Option<String>,
    /// Session status filter, for example `busy`.
    pub status: Option<String>,
    /// Only bookmarked sessions when true.
    pub bookmarked: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionMetadata>,
    /// `null` on the last page.
    pub next_cursor: Option<crate::repositories::session_repository::SessionListCursor>,
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

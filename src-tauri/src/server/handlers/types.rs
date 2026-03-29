pub use crate::agent::types::CreateSessionRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub source: Option<String>,
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
    pub request_id: String,
    pub behavior: String,
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
    pub request_id: String,
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

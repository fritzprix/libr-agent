use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub name: Option<String>,
    pub assistant_id: String, // Replaces agent_config
    pub workspace_path: Option<String>,
    pub request: String,
    pub parent_session_id: Option<String>,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub status: String, // "processed" or "queued"
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLineageMeta {
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildSessionsResponse {
    pub parent_session_id: String,
    pub count: usize,
    pub children: Vec<String>,
}

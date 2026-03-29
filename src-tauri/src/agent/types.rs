use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLineageMeta {
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

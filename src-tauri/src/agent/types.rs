use crate::execution_mode::ExecutionMode;
use crate::models::workspace_isolation::{DockerWorkspaceConfig, WorkspaceIsolationMode};
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
    pub assistant_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace_isolation: Option<WorkspaceIsolationMode>,
    pub docker_config: Option<DockerWorkspaceConfig>,
    /// Tool approval mode for the new session (`normal` | `yolo` | `unsafe`).
    /// Applied before the initial workflow starts. When omitted, inherits a
    /// non-normal parent mode when spawning a child; otherwise defaults to `normal`.
    #[serde(default)]
    pub execution_mode: Option<ExecutionMode>,
    /// Initial user message. When omitted or blank, the session is created idle
    /// without starting a workflow (useful for API smoke / mode checks).
    #[serde(default)]
    pub request: Option<String>,
    pub parent_session_id: Option<String>,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_root_session_id: Option<String>,
    /// Optional per-session override for context-management `maxInputContext`.
    ///
    /// When set (> 0), only this session's compaction / orchestration path uses
    /// the value. It is stored on the in-memory `AgentSession` and **never**
    /// written to the global settings repository / UI `maxInputContext`.
    /// Ephemeral — not persisted across restart; intended for Harbor trials.
    #[serde(default)]
    pub max_input_context: Option<u64>,
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
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_root_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLineageMeta {
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_root_session_id: Option<String>,
}

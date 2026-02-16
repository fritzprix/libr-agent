use crate::agent::types::AgentMessageDto;
use serde::{Deserialize, Serialize};

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
    pub message: AgentMessageDto,
}

/// Request to send a user message to trigger workflow
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub session_id: String,
    pub message: AgentMessageDto,
}

/// Request to inject messages silently or with workflow trigger
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMessagesRequest {
    pub session_id: String,
    pub messages: Vec<AgentMessageDto>,
    pub trigger_workflow: bool,
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

/// Tool execution result from frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub success: bool,
    pub content: String,
    pub mcp_content: Option<Vec<crate::mcp::types::MCPContent>>,
    pub error: Option<String>,
    pub is_error: bool,
}

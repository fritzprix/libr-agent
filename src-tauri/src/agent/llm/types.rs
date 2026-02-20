use crate::commands::messages_commands::Message;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRequest {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub model: String,
    pub provider: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub available_tools: Option<Vec<crate::mcp::types::MCPTool>>,
}

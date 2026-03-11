use crate::models::chat::Message;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRequest {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub model: String,
    pub provider: String,
    /// Stable system prompt (sections 1–3: agent identity, workspace instructions, session
    /// context). Cacheable across turns within a session.
    pub system_prompt: Option<String>,
    /// Volatile session context (sections 4–5: context providers + service tool states).
    /// Rebuilt on every LLM call. The frontend AI service layer decides how to inject this
    /// via `prepareContextInjection` — providers may append it to the system prompt (default)
    /// or inject it as an ephemeral message for better prefix-cache utilization.
    pub session_context: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub available_tools: Option<Vec<crate::mcp::types::MCPTool>>,
}

use crate::models::chat::Message;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentRuntimeErrorType {
    McpError,
    ToolExecutionError,
    AiServiceError,
    NetworkError,
    ValidationError,
    RateLimitError,
    MalformedFunctionCall,
    JsonParsingError,
    AuthenticationError,
    ContextLimitError,
    EmptySelectionError,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeErrorDetails {
    pub original_error: Value,
    pub error_code: Option<String>,
    pub timestamp: String,
    pub context: Option<serde_json::Map<String, Value>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeError {
    #[serde(rename = "type")]
    pub error_type: AgentRuntimeErrorType,
    pub display_message: String,
    pub recoverable: bool,
    pub details: Option<AgentRuntimeErrorDetails>,
}

impl AgentRuntimeError {
    fn ensure_details(&mut self) -> &mut AgentRuntimeErrorDetails {
        let display_message = self.display_message.clone();
        self.details
            .get_or_insert_with(|| AgentRuntimeErrorDetails {
                original_error: json!(display_message),
                error_code: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                context: None,
            })
    }

    pub fn new(error_type: AgentRuntimeErrorType, display_message: impl Into<String>) -> Self {
        Self {
            error_type,
            display_message: display_message.into(),
            recoverable: true,
            details: None,
        }
    }

    pub fn with_code(mut self, error_code: impl Into<String>) -> Self {
        let details = self.ensure_details();
        details.error_code = Some(error_code.into());
        self
    }

    pub fn with_original_error(mut self, original_error: impl Into<Value>) -> Self {
        let details = self.ensure_details();
        details.original_error = original_error.into();
        self
    }

    pub fn with_context(mut self, context: serde_json::Map<String, Value>) -> Self {
        let details = self.ensure_details();
        details.context = Some(context);
        self
    }
}

impl std::fmt::Display for AgentRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_message)
    }
}

impl From<String> for AgentRuntimeError {
    fn from(value: String) -> Self {
        AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, value.clone())
            .with_original_error(json!(value))
            .with_code("GENERIC_WORKFLOW_ERROR")
    }
}

impl From<&str> for AgentRuntimeError {
    fn from(value: &str) -> Self {
        AgentRuntimeError::from(value.to_string())
    }
}

impl From<AgentRuntimeError> for String {
    fn from(value: AgentRuntimeError) -> Self {
        value.display_message
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRequest {
    pub session_id: String,
    pub response_message_id: String,
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
    /// Token usage gauge telemetry to drive frontend UI (e.g. context window fill bar).
    pub context_usage: Option<serde_json::Value>,
}

/// Event payload emitted as `llm:compact-request`.
/// The frontend listener calls the LLM for a summary and returns via `agent_handle_compact_response`.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactRequest {
    pub session_id: String,
    /// Human-readable session name for toast display. Falls back to a short session ID prefix.
    pub session_name: String,
    /// Messages to summarize (fromId..=toId inclusive)
    pub messages: Vec<Message>,
    pub from_id: String,
    pub to_id: String,
    /// When true, Rust is waiting for this compaction to complete before retrying
    /// the blocked LLM turn.
    pub resume_completion_after_compact: bool,
}

/// Event payload emitted as `llm:compact-state`.
/// Frontend uses this to block user input while compaction is in-flight.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompactStatePhase {
    Started,
    Succeeded,
    Failed,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactStateEvent {
    pub session_id: String,
    pub session_name: Option<String>,
    pub compacting: bool,
    pub phase: CompactStatePhase,
}

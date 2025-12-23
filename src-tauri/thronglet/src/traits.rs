use crate::error::AgentError;
use crate::models::{LLMResponse, Message, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Request text generation from the LLM.
    /// The `system_prompt` is passed explicitly to allow dynamic overrides.
    async fn generate(
        &self,
        history: Vec<Message>,
        system_prompt: String,
    ) -> Result<LLMResponse, AgentError>;
}

#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Execute a tool by name.
    /// The `server_name` must be resolved by the caller (Agent) before calling this
    /// or encoded in the `tool_name` (e.g. "server__tool").
    /// Thronglet design: tool_name IS unique (prefixed) so server_name might be redundant
    /// depending on implementation, but we keep it flexible.
    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<ToolResult, AgentError>;

    /// List available tools from the specified servers.
    async fn list_tools(
        &self,
        server_names: Vec<String>,
    ) -> Result<Vec<ToolDefinition>, AgentError>;
}

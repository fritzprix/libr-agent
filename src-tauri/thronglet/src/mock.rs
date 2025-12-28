use crate::error::AgentError;
use crate::models::{LLMResponse, Message, ToolDefinition, ToolResult};
use crate::traits::{LLMProvider, ToolProvider};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockLLMProvider {
    // We use a simple FIFO queue for strict ordering of expected prompts/responses
    responses: Arc<Mutex<Vec<(String, LLMResponse)>>>,
}

impl Default for MockLLMProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLLMProvider {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push_response(&self, expected_prompt_part: &str, response: LLMResponse) {
        self.responses
            .lock()
            .unwrap()
            .push((expected_prompt_part.to_string(), response));
    }
}

#[async_trait]
impl LLMProvider for MockLLMProvider {
    async fn generate(
        &self,
        history: Vec<Message>,
        _system_prompt: String,
    ) -> Result<LLMResponse, AgentError> {
        let mut responses = self.responses.lock().unwrap();

        if responses.is_empty() {
            return Err(AgentError::LLMError(
                "No more mock responses available".to_string(),
            ));
        }

        // Get the last user message to verify context
        let last_message = history
            .last()
            .ok_or(AgentError::LLMError("History empty".to_string()))?;

        // Very basic verification: check if the first queued response matches the current situation
        // In a real mock, we might inspect 'history' more deeply.
        let (expected_part, response) = responses.remove(0);

        // Verify prompt contains expected text
        // (Simplified logic: just check if ANY user message contains the text for now, or the last one)
        // For strictness, let's say the last user message must contain it.
        let last_text = match last_message {
            Message::User { content } => content
                .iter()
                .filter_map(|c| match c {
                    crate::models::Content::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<String>(),
            Message::Tool { .. } => "TOOL_RESULT".to_string(), // Simplified match for tool output loop
            _ => String::new(),
        };

        if !expected_part.is_empty()
            && !last_text.contains(&expected_part)
            && last_text != "TOOL_RESULT"
        {
            // Relaxed check for tool loops, assuming verify elsewhere or strict test setup
            // If expected "Calculate", but getting "TOOL_RESULT", maybe valid if we queued that way.
            // For this recursive test, we usually queue [Prompt->Call], [ToolResult->Final]
            if !last_text.contains(&expected_part) {
                return Err(AgentError::LLMError(format!(
                    "Unexpected prompt: '{}', expected part: '{}'",
                    last_text, expected_part
                )));
            }
        }

        Ok(response)
    }
}

#[derive(Clone)]
pub struct MockToolProvider {
    // Map "server:tool" -> Result
    tools: Arc<Mutex<std::collections::HashMap<String, ToolResult>>>,
    // Track calls for verification
    pub calls: Arc<Mutex<Vec<(String, String, Value)>>>,
}

impl Default for MockToolProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockToolProvider {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(Mutex::new(std::collections::HashMap::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register_tool(&self, server: &str, tool: &str, result: ToolResult) {
        let key = format!("{}::{}", server, tool);
        self.tools.lock().unwrap().insert(key, result);
    }
}

#[async_trait]
impl ToolProvider for MockToolProvider {
    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<ToolResult, AgentError> {
        self.calls
            .lock()
            .unwrap()
            .push((server_name.to_string(), tool_name.to_string(), args));

        let key = format!("{}::{}", server_name, tool_name);
        let tools = self.tools.lock().unwrap();

        if let Some(res) = tools.get(&key) {
            // Clone the result but update the ID to match valid flow if needed,
            // or just return as is (assuming mock result has generic ID)
            // Ideally we should inject the request ID into the result here.
            Ok(res.clone())
        } else {
            Err(AgentError::ToolError(format!(
                "Mock tool not found: {}",
                key
            )))
        }
    }

    async fn list_tools(
        &self,
        _server_names: Vec<String>,
    ) -> Result<Vec<ToolDefinition>, AgentError> {
        Ok(Vec::new()) // Mock empty discovery for now
    }
}

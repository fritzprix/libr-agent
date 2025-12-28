use crate::error::AgentError;
use crate::models::{AgentConfig, Content, Message, ToolCall, ToolResult};
use crate::traits::{LLMProvider, ToolProvider};
use log::{debug, info, warn};
use std::sync::Arc;

pub struct Agent<L, T> {
    config: AgentConfig,
    llm_provider: Arc<L>,
    tool_provider: Arc<T>,
    history: Vec<Message>,
}

impl<L, T> Agent<L, T>
where
    L: LLMProvider,
    T: ToolProvider,
{
    pub fn new(config: AgentConfig, llm: Arc<L>, tool: Arc<T>) -> Self {
        let history = config.initial_history.clone().unwrap_or_default();
        Self {
            config,
            llm_provider: llm,
            tool_provider: tool,
            history,
        }
    }

    /// Appends a user message and runs the agent loop until completion (answer or error).
    pub async fn input(&mut self, text: String) -> Result<String, AgentError> {
        info!("Agent input received: {}", text);

        let user_msg = Message::User {
            content: vec![Content::Text { text }],
        };
        self.history.push(user_msg);

        // Start the think-act loop
        self.run_loop().await
    }

    /// The core Recursive Tool Loop
    async fn run_loop(&mut self) -> Result<String, AgentError> {
        loop {
            // 1. Generate (Think)
            debug!("Requesting LLM generation...");
            let response = self
                .llm_provider
                .generate(self.history.clone(), self.config.system_prompt.clone())
                .await?;

            // 2. Process Response
            let assistant_msg = Message::Assistant {
                content: response.content.clone(),
                tool_calls: response.tool_calls.clone(),
            };
            self.history.push(assistant_msg);

            // 3. Extract text for final answer (if any)
            let mut final_answer = String::new();
            for content in &response.content {
                if let Content::Text { text } = content {
                    final_answer.push_str(text);
                }
            }

            // 4. Check for Tool Calls
            if let Some(tool_calls) = &response.tool_calls {
                if tool_calls.is_empty() {
                    // No tools, just text -> Done
                    return Ok(final_answer);
                }

                info!("Executing {} tool calls...", tool_calls.len());

                // 5. Execute Tools (Act)
                // We execute sequentially for simplicity (can be parallelized later)
                for call in tool_calls {
                    let result = self.execute_tool(call).await;

                    // 6. Observe
                    let tool_msg = match result {
                        Ok(res) => Message::Tool {
                            tool_call_id: res.tool_call_id,
                            content: res.content,
                            is_error: Some(res.is_error),
                        },
                        Err(e) => {
                            warn!("Tool execution failed: {}", e);
                            Message::Tool {
                                tool_call_id: call.id.clone(),
                                content: vec![Content::Text {
                                    text: format!("Error: {}", e),
                                }],
                                is_error: Some(true),
                            }
                        }
                    };
                    self.history.push(tool_msg);
                }

                // 7. Recurse (Loop continues)
                info!("Tool execution finished, recursing...");
                continue;
            } else {
                // No tool calls field -> Done
                return Ok(final_answer);
            }
        }
    }

    async fn execute_tool(&self, call: &ToolCall) -> Result<ToolResult, AgentError> {
        let server_tool_pair: Vec<&str> = call.function.name.split("__").collect();

        // Unified Routing Logic:
        // Expected format: "server_name__tool_name" or just "tool_name" (needs fallback?)
        // Thronglet assumes "server__tool" convention for unambiguous routing.

        let (server_name, tool_name) = if server_tool_pair.len() >= 2 {
            (server_tool_pair[0], server_tool_pair[1])
        } else {
            // Fallback: If no server prefix, maybe allow a default?
            // Or return error. For strictness, error.
            return Err(AgentError::ToolError(format!(
                "Invalid tool name format (expected server__tool): {}",
                call.function.name
            )));
        };

        let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
            .map_err(|e| AgentError::ToolError(format!("Failed to parse arguments: {}", e)))?;

        self.tool_provider
            .call_tool(server_name, tool_name, args)
            .await
    }

    pub fn history(&self) -> &Vec<Message> {
        &self.history
    }
}

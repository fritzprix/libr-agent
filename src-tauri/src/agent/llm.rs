use crate::mcp::service_proxy::MCPServiceProxy;
use std::sync::Arc;

/// Build complete system prompt for session
///
/// Combines:
/// - Agent base prompt (from agent_config.system_prompt)
/// - Built-in service contexts (Planning, Knowledge, ContentStore, Workspace)
pub async fn build_system_prompt(
    agent_config: &crate::agent::AgentConfig,
    proxy: Option<Arc<MCPServiceProxy>>,
) -> Result<String, String> {
    let mut parts = Vec::new();

    // 1. Load Agent base prompt
    // Note: Caller must provide the parsed AgentConfig
    if !agent_config.system_prompt.trim().is_empty() {
        parts.push(agent_config.system_prompt.clone());
    }

    // 2. Get Built-in service contexts (best-effort)
    if let Some(p) = proxy {
        let contexts: std::collections::HashMap<String, String> = p.get_service_contexts().await;

        if !contexts.is_empty() {
            parts.push("\n\n## Available Tools & Current State\n".to_string());

            for (_tool_id, context_prompt) in contexts {
                if !context_prompt.trim().is_empty() {
                    parts.push(context_prompt);
                }
            }
        }
    }

    let final_prompt = parts.join("\n");
    // Caller should log the prompt if needed

    Ok(final_prompt)
}

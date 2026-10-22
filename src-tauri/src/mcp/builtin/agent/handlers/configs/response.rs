use serde_json::{json, Value};

pub(super) fn extract_string_array(config: &Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn format_summary_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

/// True when optional builtins and external MCP servers are both empty (core-only).
pub fn config_needs_tool_configuration(config: &Value) -> bool {
    let configured_builtins = extract_string_array(config, "allowedBuiltInServiceAliases");
    let external_mcp_servers = extract_string_array(config, "mcpServerIds");
    configured_builtins.is_empty() && external_mcp_servers.is_empty()
}

pub fn create_agent_follow_ups(id: &str, needs_tools: bool) -> Vec<String> {
    if needs_tools {
        vec![
            "Call tool__listServers(availability=\"inventory\") to copy real MCP server IDs — do not invent names like 'exa'.".to_string(),
            format!(
                "Call agent__updateAgent(id=\"{id}\", builtinCapabilities=[\"browser\",\"knowledge\"] when research, externalMcpServers=[...ids from listServers])."
            ),
            "Only then call agent__spawnSession(configId=...) — spawning now leaves this member on core-only tools.".to_string(),
        ]
    } else {
        vec![
            "List agent configurations to review the new configuration".to_string(),
            "Update the configuration if you want to refine its prompt or capabilities".to_string(),
        ]
    }
}

pub fn create_agent_next_actions(id: &str, needs_tools: bool) -> Vec<Value> {
    if needs_tools {
        vec![
            json!({
                "toolName": "tool__listServers",
                "reason": "List inventory MCP server IDs before configuring externalMcpServers on this agent.",
                "args": {
                    "availability": "inventory"
                }
            }),
            json!({
                "toolName": "agent__updateAgent",
                "reason": "Fill externalMcpServers with IDs from listServers; adjust builtinCapabilities to the role before spawnSession.",
                "args": {
                    "id": id,
                    "builtinCapabilities": ["browser", "knowledge"],
                    "externalMcpServers": []
                }
            }),
        ]
    } else {
        vec![json!({
            "toolName": "agent__listAgents",
            "reason": "Review the available agent configurations after creating this one.",
            "args": {
                "type": "configs"
            }
        })]
    }
}

pub fn build_agent_config_response_data(id: &str, name: &str, config: &Value) -> Value {
    let configured_builtin_capabilities =
        extract_string_array(config, "allowedBuiltInServiceAliases");
    let effective_builtin_capabilities =
        crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(config);
    let external_mcp_servers = extract_string_array(config, "mcpServerIds");
    let needs_tool_configuration = config_needs_tool_configuration(config);
    let tool_access = if needs_tool_configuration {
        "core-only"
    } else {
        "configured"
    };

    json!({
        "success": true,
        "id": id,
        "name": name,
        "description": config.get("description").and_then(Value::as_str),
        "systemPrompt": config.get("systemPrompt").and_then(Value::as_str),
        "builtinCapabilities": effective_builtin_capabilities.clone(),
        "configuredBuiltinCapabilities": configured_builtin_capabilities.clone(),
        "effectiveBuiltinCapabilities": effective_builtin_capabilities,
        "externalMcpServers": external_mcp_servers.clone(),
        "mcpServerIds": external_mcp_servers,
        "needsToolConfiguration": needs_tool_configuration,
        "toolAccess": tool_access,
    })
}

pub fn build_agent_config_echo_message(
    action: &str,
    name: &str,
    id: &str,
    config: &Value,
) -> String {
    let configured_builtin_capabilities =
        extract_string_array(config, "allowedBuiltInServiceAliases");
    let effective_builtin_capabilities =
        crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(config);
    let external_mcp_servers = extract_string_array(config, "mcpServerIds");
    let description = config
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let mut message = format!(
        "Agent configuration '{}' {} (ID: {})\n\nDescription: {}\nConfigured builtin capabilities: {}\nEffective builtin capabilities: {}\nExternal MCP servers: {}",
        name,
        action,
        id,
        description,
        format_summary_list(&configured_builtin_capabilities),
        format_summary_list(&effective_builtin_capabilities),
        format_summary_list(&external_mcp_servers),
    );

    if config_needs_tool_configuration(config) {
        message.push_str(
            "\nTool access: core-only — configure optional tools before spawnSession if this agent needs research/browse/MCP.",
        );
    }

    message
}

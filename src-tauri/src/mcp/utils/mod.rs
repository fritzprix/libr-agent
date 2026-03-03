pub mod command_helper;
pub mod env;
pub mod schema_builder;

/// Serialize a list of `rmcp::model::Tool` (raw MCP protocol type) to a compact JSON cache string.
/// Used by `test_server_connection` in `mcp_manager/operations.rs`.
pub fn serialize_rmcp_tools(tools: &[rmcp::model::Tool]) -> String {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description.as_deref().unwrap_or("")
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Serialize a list of `crate::mcp::types::MCPTool` (our internal type) to a compact JSON cache string.
/// Used by `probe_mcp_server` in `mcp_commands.rs`.
pub fn serialize_mcp_tools(tools: &[crate::mcp::types::MCPTool]) -> String {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::schema::JSONSchema;
    use crate::mcp::types::MCPTool;

    fn make_tool(name: &str, description: &str) -> MCPTool {
        MCPTool {
            name: name.to_string(),
            title: None,
            description: description.to_string(),
            input_schema: JSONSchema::null(),
            output_schema: None,
            annotations: None,
        }
    }

    #[test]
    fn test_serialize_mcp_tools_empty() {
        assert_eq!(serialize_mcp_tools(&[]), "[]");
    }

    #[test]
    fn test_serialize_mcp_tools_with_entries() {
        let tools = vec![
            make_tool("doThing", "Does the thing"),
            make_tool("otherTool", "Another tool"),
        ];
        let json = serialize_mcp_tools(&tools);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["name"], "doThing");
        assert_eq!(parsed[0]["description"], "Does the thing");
        assert_eq!(parsed[1]["name"], "otherTool");
    }

    #[test]
    fn test_serialize_mcp_tools_output_is_valid_cache_format() {
        // Regression: output must be parseable as Vec<{name, description}> for list_servers
        let tools = vec![make_tool("myTool", "desc")];
        let json_str = serialize_mcp_tools(&tools);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert!(parsed[0].get("name").is_some());
        assert!(parsed[0].get("description").is_some());
    }
}

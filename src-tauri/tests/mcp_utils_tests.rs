/// Integration tests for mcp::utils serialization helpers.
///
/// These tests replace #[cfg(test)] unit tests that cannot run via `cargo test --lib`
/// on Windows (STATUS_ENTRYPOINT_NOT_FOUND DLL issue). CI uses `cargo test --tests`.
use tauri_mcp_agent_lib::mcp::schema::JSONSchema;
use tauri_mcp_agent_lib::mcp::types::MCPTool;
use tauri_mcp_agent_lib::mcp::utils::serialize_mcp_tools;

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
    // Regression: output must be parseable as Vec<{name, description}> for list_servers display.
    let tools = vec![make_tool("myTool", "desc")];
    let json_str = serialize_mcp_tools(&tools);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert!(parsed[0].get("name").is_some());
    assert!(parsed[0].get("description").is_some());
}

/// Integration tests for mcp::utils serialization helpers.
///
/// These tests replace #[cfg(test)] unit tests that cannot run via `cargo test --lib`
/// on Windows (STATUS_ENTRYPOINT_NOT_FOUND DLL issue). CI uses `cargo test --tests`.
use serde_json::json;
use tauri_mcp_agent_lib::mcp::schema::{
    JSONSchema, JSONSchemaAdditionalProperties, JSONSchemaType,
};
use tauri_mcp_agent_lib::mcp::server_utils::convert_input_schema;
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

#[test]
fn test_convert_input_schema_preserves_object_form_additional_properties() {
    let raw_schema = json!({
        "type": "object",
        "properties": {
            "prompt": {
                "type": "object",
                "propertyNames": { "type": "string" },
                "additionalProperties": {},
                "description": "Workflow in ComfyUI API JSON format."
            },
            "client_id": {
                "type": "string",
                "description": "Optional unique client identifier."
            }
        },
        "required": ["prompt"],
        "additionalProperties": false
    });

    let converted = convert_input_schema(raw_schema);

    let JSONSchemaType::Object {
        properties,
        required,
        additional_properties,
        ..
    } = &converted.schema_type
    else {
        panic!("top-level schema should stay an object");
    };

    assert_eq!(required.as_ref(), Some(&vec!["prompt".to_string()]));
    assert_eq!(
        additional_properties.as_ref(),
        Some(&JSONSchemaAdditionalProperties::Boolean(false))
    );

    let prompt_schema = properties
        .as_ref()
        .and_then(|props| props.get("prompt"))
        .expect("prompt property should exist");

    let JSONSchemaType::Object {
        additional_properties,
        property_names,
        ..
    } = &prompt_schema.schema_type
    else {
        panic!("prompt schema should stay an object");
    };

    assert_eq!(
        additional_properties.as_ref(),
        Some(&JSONSchemaAdditionalProperties::Schema(json!({})))
    );
    assert_eq!(property_names.as_ref(), Some(&json!({ "type": "string" })));

    let serialized = serde_json::to_value(&converted).expect("schema should serialize");
    assert_eq!(serialized["type"], "object");
    assert_eq!(
        serialized["properties"]["prompt"]["additionalProperties"],
        json!({})
    );
    assert_eq!(
        serialized["properties"]["prompt"]["propertyNames"],
        json!({ "type": "string" })
    );
}

use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;
use tauri_mcp_agent_lib::mcp::types::MCPTool;

fn object_property_keys(schema: &tauri_mcp_agent_lib::mcp::schema::JSONSchema) -> Vec<String> {
    let JSONSchemaType::Object {
        properties: Some(properties),
        ..
    } = &schema.schema_type
    else {
        panic!("expected object schema");
    };

    properties.keys().cloned().collect()
}

fn serialized_property_keys(schema: &tauri_mcp_agent_lib::mcp::schema::JSONSchema) -> Vec<String> {
    let serialized = serde_json::to_value(schema).expect("serialize schema");
    serialized["properties"]
        .as_object()
        .expect("properties object")
        .keys()
        .cloned()
        .collect()
}

fn assert_property_order(tool: &MCPTool, expected: &[&str]) {
    let keys = object_property_keys(&tool.input_schema);
    assert_eq!(
        keys,
        expected
            .iter()
            .map(|key| key.to_string())
            .collect::<Vec<_>>(),
        "{} property order mismatch",
        tool.name
    );

    let serialized_keys = serialized_property_keys(&tool.input_schema);
    assert_eq!(
        serialized_keys,
        expected
            .iter()
            .map(|key| key.to_string())
            .collect::<Vec<_>>(),
        "{} serialized property order mismatch",
        tool.name
    );
}

fn edit_variant_property_keys(
    schema: &tauri_mcp_agent_lib::mcp::schema::JSONSchema,
    description_contains: &str,
) -> Vec<String> {
    let variants = schema.one_of.as_ref().expect("expected oneOf variants");

    let variant = variants
        .iter()
        .find(|variant| {
            variant
                .description
                .as_deref()
                .is_some_and(|description| description.contains(description_contains))
        })
        .unwrap_or_else(|| panic!("missing edit variant: {description_contains}"));

    object_property_keys(variant)
}

#[test]
fn present_interactive_schema_property_order_is_format_title_interaction_content() {
    use tauri_mcp_agent_lib::mcp::builtin::ui::tools::present_interactive_tool;

    let tool = present_interactive_tool();
    assert_property_order(&tool, &["format", "title", "interaction", "content"]);
}

#[test]
fn report_result_schema_property_order_puts_result_last() {
    use tauri_mcp_agent_lib::mcp::builtin::ui::tools::report_result_tool;

    let tool = report_result_tool();
    assert_property_order(&tool, &["status", "format", "title", "result"]);
}

#[test]
fn record_knowledge_schema_property_order_puts_content_last() {
    use tauri_mcp_agent_lib::mcp::builtin::knowledge::tools::record_knowledge_tool;

    let tool = record_knowledge_tool();
    assert_property_order(
        &tool,
        &[
            "tags",
            "source",
            "auto_extract",
            "entities",
            "relationships",
            "content",
        ],
    );
}

#[test]
fn create_agent_schema_property_order_puts_system_prompt_last() {
    use tauri_mcp_agent_lib::mcp::builtin::agent::tools::all_tools;

    let tool = all_tools()
        .into_iter()
        .find(|tool| tool.name == "createAgent")
        .expect("createAgent tool");

    assert_property_order(
        &tool,
        &[
            "name",
            "description",
            "temperature",
            "builtinCapabilities",
            "externalMcpServers",
            "systemPrompt",
        ],
    );
}

#[test]
fn update_agent_schema_property_order_puts_system_prompt_last() {
    use tauri_mcp_agent_lib::mcp::builtin::agent::tools::all_tools;

    let tool = all_tools()
        .into_iter()
        .find(|tool| tool.name == "updateAgent")
        .expect("updateAgent tool");

    assert_property_order(
        &tool,
        &[
            "id",
            "name",
            "description",
            "temperature",
            "builtinCapabilities",
            "externalMcpServers",
            "systemPrompt",
        ],
    );
}

#[test]
fn start_session_schema_property_order_puts_task_last() {
    use tauri_mcp_agent_lib::mcp::builtin::agent::tools::all_tools;

    let tool = all_tools()
        .into_iter()
        .find(|tool| tool.name == "startSession")
        .expect("startSession tool");

    assert_property_order(
        &tool,
        &[
            "agentId",
            "workspaceOverride",
            "waitForResult",
            "timeout",
            "task",
        ],
    );
}

#[cfg(feature = "workspace-edit-file")]
#[test]
fn edit_file_line_variant_property_order_puts_content_last() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_edit_file_input_schema;

    let keys = edit_variant_property_keys(
        &create_edit_file_input_schema(),
        "Replace or delete existing lines",
    );

    assert_eq!(
        keys,
        vec![
            "path".to_string(),
            "start".to_string(),
            "end".to_string(),
            "op".to_string(),
            "content".to_string(),
        ]
    );
}

#[cfg(feature = "workspace-edit-file")]
#[test]
fn edit_file_prepend_variant_property_order_puts_content_last() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_edit_file_input_schema;

    let keys = edit_variant_property_keys(
        &create_edit_file_input_schema(),
        "Prepend content at the top of the file",
    );

    assert_eq!(
        keys,
        vec![
            "path".to_string(),
            "start".to_string(),
            "content".to_string(),
        ]
    );
}

#[cfg(feature = "workspace-edit-file")]
#[test]
fn edit_file_insert_after_variant_property_order_puts_content_last() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_edit_file_input_schema;

    let keys = edit_variant_property_keys(
        &create_edit_file_input_schema(),
        "Insert content after an existing line",
    );

    assert_eq!(
        keys,
        vec![
            "path".to_string(),
            "op".to_string(),
            "start".to_string(),
            "content".to_string(),
        ]
    );
}

#[cfg(feature = "workspace-str-replace")]
#[test]
fn str_replace_schema_property_order_puts_text_fields_last() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_str_replace_tool;

    let tool = create_str_replace_tool();
    assert_property_order(&tool, &["path", "replace_all", "old_string", "new_string"]);
}

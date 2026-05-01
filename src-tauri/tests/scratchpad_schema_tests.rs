use tauri_mcp_agent_lib::mcp::builtin::scratchpad::ScratchpadServer;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;

fn scratchpad_tool(name: &str) -> tauri_mcp_agent_lib::mcp::MCPTool {
    ScratchpadServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} tool should exist"))
}

fn object_properties(
    tool: &tauri_mcp_agent_lib::mcp::MCPTool,
) -> &std::collections::HashMap<String, tauri_mcp_agent_lib::mcp::schema::JSONSchema> {
    match &tool.input_schema.schema_type {
        JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties,
        other => panic!("expected object schema, got {other:?}"),
    }
}

#[test]
fn scratchpad_list_schema_uses_open_ended_positive_pagination_bounds() {
    let list_tool = scratchpad_tool("list");
    let properties = object_properties(&list_tool);

    let limit_schema = properties.get("limit").expect("list should expose limit");
    match &limit_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(*minimum, Some(10));
            assert_eq!(*maximum, None);
        }
        other => panic!("expected integer schema, got {other:?}"),
    }

    let offset_schema = properties
        .get("offset")
        .expect("list should expose offset");
    match &offset_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(*minimum, Some(0));
            assert_eq!(*maximum, None);
        }
        other => panic!("expected integer schema, got {other:?}"),
    }
}

#[test]
fn scratchpad_read_schema_leaves_note_ids_unbounded() {
    let read_tool = scratchpad_tool("read");
    let properties = object_properties(&read_tool);
    let ids_schema = properties.get("ids").expect("read should expose ids");

    let item_schema = match &ids_schema.schema_type {
        JSONSchemaType::Array { items, .. } => {
            items.as_ref().expect("ids should define item schema")
        }
        other => panic!("expected array schema, got {other:?}"),
    };

    match &item_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(*minimum, None);
            assert_eq!(*maximum, None);
        }
        other => panic!("expected integer schema, got {other:?}"),
    }
}

#[test]
fn scratchpad_clear_schema_leaves_note_id_unbounded() {
    let clear_tool = scratchpad_tool("clear");
    let properties = object_properties(&clear_tool);
    let id_schema = properties.get("id").expect("clear should expose id");

    match &id_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(*minimum, None);
            assert_eq!(*maximum, None);
        }
        other => panic!("expected integer schema, got {other:?}"),
    }
}

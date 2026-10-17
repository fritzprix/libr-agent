//! Coverage for core-only createAgent follow-up / next-action guidance.
//!
//! Lib unit tests are disabled (`[lib] test = false`); keep these pure-helper
//! checks in a dedicated integration binary so CI does not need the monolith.

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    build_agent_config_echo_message, build_agent_config_response_data,
    config_needs_tool_configuration, create_agent_follow_ups, create_agent_next_actions,
};
use tauri_mcp_agent_lib::mcp::builtin::agent::tools as agent_tools;

#[test]
fn empty_config_needs_tool_configuration() {
    assert!(config_needs_tool_configuration(&json!({})));
    assert!(config_needs_tool_configuration(&json!({
        "allowedBuiltInServiceAliases": [],
        "mcpServerIds": []
    })));
}

#[test]
fn builtins_or_mcp_clear_needs_tool_configuration() {
    assert!(!config_needs_tool_configuration(&json!({
        "allowedBuiltInServiceAliases": ["browser"]
    })));
    assert!(!config_needs_tool_configuration(&json!({
        "mcpServerIds": ["server-1"]
    })));
    assert!(!config_needs_tool_configuration(&json!({
        "allowedBuiltInServiceAliases": ["knowledge"],
        "mcpServerIds": ["server-1"]
    })));
}

#[test]
fn core_only_follow_ups_steer_to_list_servers_then_update() {
    let id = "cfg-abc";
    let follow_ups = create_agent_follow_ups(id, true);
    assert_eq!(follow_ups.len(), 3);
    assert!(follow_ups[0].contains("tool__listServers"));
    assert!(follow_ups[1].contains("agent__updateAgent"));
    assert!(follow_ups[1].contains(id));
    assert!(follow_ups[2].contains("spawnSession"));

    let soft = create_agent_follow_ups(id, false);
    assert!(soft.iter().all(|line| !line.contains("tool__listServers")));
    assert!(soft
        .iter()
        .any(|line| line.contains("List agent configurations")));
}

#[test]
fn core_only_next_actions_prioritize_list_servers_then_update_with_id() {
    let id = "cfg-abc";
    let actions = create_agent_next_actions(id, true);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["toolName"], "tool__listServers");
    assert_eq!(actions[0]["args"]["availability"], "inventory");
    assert_eq!(actions[1]["toolName"], "agent__updateAgent");
    assert_eq!(actions[1]["args"]["id"], id);
    assert!(actions[1]["args"]["builtinCapabilities"].is_array());
    assert!(actions[1]["args"]["externalMcpServers"].is_array());

    let configured = create_agent_next_actions(id, false);
    assert_eq!(configured.len(), 1);
    assert_eq!(configured[0]["toolName"], "agent__listAgents");
}

#[test]
fn response_data_flags_core_only_vs_configured() {
    let core_only = build_agent_config_response_data(
        "id-1",
        "Tour",
        &json!({
            "allowedBuiltInServiceAliases": [],
            "description": "coord"
        }),
    );
    assert_eq!(core_only["needsToolConfiguration"], true);
    assert_eq!(core_only["toolAccess"], "core-only");

    let configured = build_agent_config_response_data(
        "id-2",
        "Tour",
        &json!({
            "allowedBuiltInServiceAliases": ["browser"],
            "mcpServerIds": ["srv"]
        }),
    );
    assert_eq!(configured["needsToolConfiguration"], false);
    assert_eq!(configured["toolAccess"], "configured");
}

#[test]
fn create_agent_schema_documents_core_only_and_update_before_spawn() {
    let tool = agent_tools::all_tools()
        .into_iter()
        .find(|tool| tool.name == "createAgent")
        .expect("createAgent tool");

    let description = match &tool.input_schema.schema_type {
        tauri_mcp_agent_lib::mcp::schema::JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties
            .get("builtinCapabilities")
            .and_then(|schema| schema.description.as_deref())
            .unwrap_or_default(),
        _ => panic!("createAgent input_schema must be an object"),
    };

    assert!(
        description.contains("only core services")
            || description.contains("only the always-on core"),
        "createAgent schema must document omit → core-only default: {description}"
    );
    assert!(
        description.contains("updateAgent") || description.contains("spawnSession"),
        "createAgent schema should steer org/research members to set tools before spawn: {description}"
    );
}

#[test]
fn echo_message_mentions_core_only_tool_access_when_empty() {
    let message = build_agent_config_echo_message(
        "created successfully",
        "Tour Coordinator",
        "id-1",
        &json!({ "allowedBuiltInServiceAliases": [] }),
    );
    assert!(message.contains("Tool access: core-only"));
    assert!(message.contains("spawnSession"));

    let configured = build_agent_config_echo_message(
        "created successfully",
        "Tour Coordinator",
        "id-1",
        &json!({ "allowedBuiltInServiceAliases": ["browser"] }),
    );
    assert!(!configured.contains("Tool access: core-only"));
}

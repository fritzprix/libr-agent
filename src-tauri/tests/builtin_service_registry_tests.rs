//! Integration tests for the builtin service registry and tool-call utilities
//! in `agent::tools`.
//!
//! These tests live in `tests/` (not `#[cfg(test)]` inside the lib) because
//! `[lib] test = false` is required for the cdylib/staticlib Tauri build.
//! Running: `cargo test --tests`

use std::collections::HashMap;

use tauri_mcp_agent_lib::agent::state::PendingToolExecution;
use tauri_mcp_agent_lib::agent::tools::{
    canonicalize_builtin_service_alias, classify_tool_result, create_error_tool_result,
    create_tool_result_message, create_tool_result_message_with_content, extract_builtin_tool_ids,
    ToolResultAcceptance,
};
use tauri_mcp_agent_lib::agent::AgentConfig;
use tauri_mcp_agent_lib::mcp::builtin::agent::tools as agent_tools;
use tauri_mcp_agent_lib::mcp::builtin::assistant::tools as assistant_tools;
use tauri_mcp_agent_lib::mcp::builtin::service_id::{
    BuiltinServiceId, BUILTIN_SERVICE_REGISTRY, CORE_BUILTIN_SERVICE_ALIASES,
};
use tauri_mcp_agent_lib::mcp::builtin::tool::tools as tool_tools;
use tauri_mcp_agent_lib::mcp::builtin::ui::tools as ui_tools;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;
use tauri_mcp_agent_lib::mcp::server::tools::{
    get_static_tools_for_server, list_available_builtin_server_definitions,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;

// ─── Test helpers ────────────────────────────────────────────────────────────

fn mock_agent_config(aliases: Option<Vec<&str>>) -> AgentConfig {
    AgentConfig {
        id: Some("assistant-test".to_string()),
        name: "Test Assistant".to_string(),
        description: None,
        system_prompt: "You are helpful".to_string(),
        mcp_server_ids: Vec::new(),
        local_services: Vec::new(),
        allowed_built_in_service_aliases: aliases.map(|values| {
            values
                .into_iter()
                .map(|v| {
                    BuiltinServiceId::from_alias(v)
                        .unwrap_or_else(|| panic!("unknown alias in test: {v}"))
                })
                .collect()
        }),
        temperature: Some(1.0),
        max_tokens: None,
        max_depth: None,
        max_fanout: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
    }
}

fn mock_pending_execution(expected: &[&str], completed: &[&str]) -> PendingToolExecution {
    PendingToolExecution {
        message_id: "msg-1".to_string(),
        total_expected: expected.len(),
        results: Vec::new(),
        tool_names: HashMap::new(),
        expected_tool_call_ids: expected.iter().map(|id| (*id).to_string()).collect(),
        completed_tool_call_ids: completed.iter().map(|id| (*id).to_string()).collect(),
    }
}

// ─── classify_tool_result tests ──────────────────────────────────────────────

#[test]
fn test_classify_tool_result_accepts_expected_unseen_id() {
    let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
    let result = classify_tool_result(&pending, "call-2");
    assert_eq!(result, ToolResultAcceptance::Accept);
}

#[test]
fn test_classify_tool_result_rejects_stale_id() {
    let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
    let result = classify_tool_result(&pending, "call-999");
    assert_eq!(result, ToolResultAcceptance::Stale);
}

#[test]
fn test_classify_tool_result_rejects_duplicate_id() {
    let pending = mock_pending_execution(&["call-1", "call-2"], &["call-1"]);
    let result = classify_tool_result(&pending, "call-1");
    assert_eq!(result, ToolResultAcceptance::Duplicate);
}

// ─── Tool result message builder tests ───────────────────────────────────────

#[test]
fn test_tool_result_with_structured_content() {
    let session_id = "test-session";
    let tool_call_id = "call-123";
    let content = vec![MCPContent::Text {
        text: "Test result".to_string(),
        is_error: None,
    }];

    let message = create_tool_result_message_with_content(session_id, tool_call_id, content, None);

    // No double wrapping
    assert_eq!(message.content.len(), 1);
    assert_eq!(message.role, "tool");
    assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));

    match &message.content[0] {
        MCPContent::Text { text, .. } => {
            assert_eq!(text, "Test result");
            assert!(!text.contains("\"content\""), "should not be JSON-wrapped");
            assert!(!text.starts_with('{'), "should not be JSON-wrapped");
        }
        _ => panic!("Expected text content"),
    }
}

#[test]
fn test_tool_result_fallback_to_string() {
    let session_id = "test-session";
    let tool_call_id = "call-123";
    let content_str = "Plain text result";

    let message =
        create_tool_result_message(session_id, tool_call_id, content_str.to_string(), None);

    assert_eq!(message.content.len(), 1);
    assert_eq!(message.role, "tool");
    assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));

    match &message.content[0] {
        MCPContent::Text { text, .. } => assert_eq!(text, content_str),
        _ => panic!("Expected text content"),
    }
}

#[test]
fn test_error_tool_result() {
    let session_id = "test-session";
    let tool_call_id = "call-123";
    let error_msg = "Tool execution failed";

    let message = create_error_tool_result(session_id, tool_call_id, error_msg, None);

    assert_eq!(message.role, "tool");
    assert_eq!(message.tool_call_id, Some(tool_call_id.to_string()));
    assert_eq!(message.content.len(), 1);

    match &message.content[0] {
        MCPContent::Text { text, .. } => assert!(text.contains(error_msg)),
        _ => panic!("Expected text content"),
    }
}

#[test]
fn test_multiple_content_items() {
    let session_id = "test-session";
    let tool_call_id = "call-123";
    let content = vec![
        MCPContent::Text {
            text: "First item".to_string(),
            is_error: None,
        },
        MCPContent::Text {
            text: "Second item".to_string(),
            is_error: None,
        },
    ];

    let message = create_tool_result_message_with_content(session_id, tool_call_id, content, None);

    assert_eq!(message.content.len(), 2);

    match &message.content[0] {
        MCPContent::Text { text, .. } => assert_eq!(text, "First item"),
        _ => panic!("Expected text content at index 0"),
    }
    match &message.content[1] {
        MCPContent::Text { text, .. } => assert_eq!(text, "Second item"),
        _ => panic!("Expected text content at index 1"),
    }
}

// ─── extract_builtin_tool_ids tests ──────────────────────────────────────────

/// Verify that core aliases are always present regardless of what the agent config says.
/// With the typed `Vec<BuiltinServiceId>` field, only valid aliases can appear in the
/// config — unknown strings are rejected at deserialisation time.
/// This test verifies that even when only optional aliases are explicitly requested,
/// all CORE aliases are still included in the result.
#[test]
fn extract_builtin_tool_ids_core_aliases_always_present_despite_unknown_inputs() {
    // Only "browser" (optional) is explicitly requested; core aliases must still appear.
    let config = mock_agent_config(Some(vec!["browser"]));
    let tool_ids = extract_builtin_tool_ids(&config);

    // These are present because they are CORE aliases, not because of alias normalization.
    assert!(
        tool_ids.contains(&"agent".to_string()),
        "agent is a core alias and must always be present"
    );
    assert!(
        tool_ids.contains(&"attachments".to_string()),
        "attachments is a core alias and must always be present"
    );
    // browser IS a recognised canonical and is optional — it's in the alias list, so it should be included.
    assert!(
        tool_ids.contains(&"browser".to_string()),
        "browser is a valid canonical alias and was explicitly requested"
    );
}

#[test]
fn extract_builtin_tool_ids_always_includes_core_aliases() {
    let config = mock_agent_config(Some(vec!["browser"]));
    let tool_ids = extract_builtin_tool_ids(&config);

    for alias in CORE_BUILTIN_SERVICE_ALIASES {
        let alias_str: &str = alias;
        assert!(
            tool_ids.contains(&alias_str.to_string()),
            "core alias {alias:?} must always be present"
        );
    }
    assert!(
        tool_ids.contains(&"browser".to_string()),
        "browser was explicitly requested and must be present"
    );
}

/// Regression: tool was registered as optional:false but omitted from
/// CORE_BUILTIN_SERVICE_ALIASES, so assistants with an explicit alias list
/// couldn't call tool tools ("Built-in server 'tool' not enabled").
/// This test ensures tool is always available even when only a single
/// unrelated optional service is requested.
#[test]
fn tool_is_always_enabled_for_any_explicit_alias_list() {
    // Only "browser" explicitly requested — tool must still be present
    // because it is a core alias.
    let config = mock_agent_config(Some(vec!["browser"]));
    let tool_ids = extract_builtin_tool_ids(&config);
    assert!(
        tool_ids.contains(&"tool".to_string()),
        "tool must always be present (it is a core alias), \
         but was missing when only 'browser' was in allowedBuiltInServiceAliases"
    );
}

#[test]
fn ui_public_surface_prefers_present_interactive_over_legacy_split_tools() {
    let tool_names: Vec<String> = ui_tools::all_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();

    assert!(
        tool_names.contains(&"presentInteractive".to_string()),
        "presentInteractive must remain on the public UI surface"
    );
    assert!(
        !tool_names.contains(&"visualizeData".to_string()),
        "visualizeData should be hidden from the AI-facing UI surface in favor of presentInteractive"
    );
    assert!(
        !tool_names.contains(&"promptUser".to_string()),
        "promptUser must not be exposed on the AI-facing UI surface"
    );
    assert!(
        !tool_names.contains(&"presentContent".to_string()),
        "presentContent must not be exposed on the AI-facing UI surface"
    );
}

#[test]
fn scheduled_task_service_is_registered_as_optional_builtin() {
    assert_eq!(
        BuiltinServiceId::from_alias("scheduled_task"),
        Some(BuiltinServiceId::ScheduledTask)
    );
    assert_eq!(
        BuiltinServiceId::from_alias("scheduled-task"),
        Some(BuiltinServiceId::ScheduledTask)
    );

    let entry = BUILTIN_SERVICE_REGISTRY
        .iter()
        .find(|entry| entry.variant == BuiltinServiceId::ScheduledTask)
        .expect("scheduled_task must be registered");

    assert_eq!(entry.canonical, "scheduled_task");
    assert!(entry.optional, "scheduled_task should remain opt-in");
    assert!(
        !CORE_BUILTIN_SERVICE_ALIASES.contains(&"scheduled_task"),
        "scheduled_task must not be treated as core"
    );
}

#[test]
fn scheduled_task_static_tool_surface_is_exposed() {
    let tools = get_static_tools_for_server("scheduled_task");
    let tool_names: Vec<String> = tools.iter().map(|tool| tool.name.clone()).collect();

    assert_eq!(
        tools.len(),
        6,
        "scheduled_task should expose six public tools"
    );
    for expected in [
        "createScheduledTask",
        "listScheduledTasks",
        "getScheduledTask",
        "updateScheduledTask",
        "toggleScheduledTask",
        "deleteScheduledTask",
    ] {
        assert!(
            tool_names.iter().any(|name| name == expected),
            "missing scheduled_task tool: {expected}"
        );
    }

    let server_info = list_available_builtin_server_definitions()
        .into_iter()
        .find(|info| info.name == "scheduled_task")
        .expect("scheduled_task must appear in builtin server definitions");
    assert_eq!(server_info.tool_count, 6);
    assert_eq!(server_info.metadata.display_name, "Scheduled Tasks");
}

#[test]
fn tool_is_enabled_even_with_empty_alias_list() {
    // Empty explicit list → only core aliases should be enabled.
    let config = mock_agent_config(Some(vec![]));
    let tool_ids = extract_builtin_tool_ids(&config);
    assert!(
        tool_ids.contains(&"tool".to_string()),
        "tool must be present even when allowedBuiltInServiceAliases is empty"
    );
}

// ─── Legacy alias migration regression tests ─────────────────────────────────

// ─── BuiltinServiceId serde stability tests ───────────────────────────────────
// Stable DB key requirement: serde value must never drift from the canonical name.
// If these fail, old DB records become unreadable.

/// Every BuiltinServiceId must serialize to its canonical name string.
/// The serialized form is the stable DB key — it must match `name()`.
#[test]
fn builtin_service_id_serializes_to_canonical_name() {
    let cases = [
        (BuiltinServiceId::Planning, "planning"),
        (BuiltinServiceId::Scratchpad, "scratchpad"),
        (BuiltinServiceId::Workspace, "workspace"),
        (BuiltinServiceId::Knowledge, "knowledge"),
        (BuiltinServiceId::History, "history"),
        (BuiltinServiceId::Agent, "agent"),
        (BuiltinServiceId::Skills, "skills"),
        (BuiltinServiceId::Playbook, "playbook"),
        (BuiltinServiceId::Attachments, "attachments"),
        (BuiltinServiceId::Ui, "ui"),
        (BuiltinServiceId::Browser, "browser"),
        (BuiltinServiceId::ScheduledTask, "scheduled_task"),
        (BuiltinServiceId::Bootstrap, "bootstrap"),
        (BuiltinServiceId::Tool, "tool"),
    ];
    for (id, expected) in cases {
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(
            json,
            format!("\"{expected}\""),
            "{id:?} must serialize to {expected:?} (stable DB key)"
        );
        // name() must agree with the serde form
        assert_eq!(id.name(), expected, "{id:?}.name() must match canonical");
    }
}

// ─── Server name / registry regression tests ─────────────────────────────────
// Original bug: ContentStoreServer::name() returned "contentstore" while the
// registry had "attachments". All four tests below would have caught it.

/// Every concrete server NAME must be a recognised canonical in the registry.
#[test]
fn each_builtin_server_name_is_in_registry() {
    use tauri_mcp_agent_lib::mcp::builtin;

    let all_names: &[&str] = &[
        builtin::planning::NAME,
        builtin::scratchpad::NAME,
        builtin::workspace::NAME,
        builtin::knowledge::NAME,
        builtin::history::NAME,
        builtin::agent::NAME,
        builtin::skills::NAME,
        builtin::playbook::NAME,
        builtin::attachments::NAME,
        builtin::ui::NAME,
        builtin::browser::NAME,
        builtin::scheduled_task::NAME,
        builtin::bootstrap::NAME,
        builtin::media::NAME,
        builtin::tool::NAME,
    ];

    for name in all_names {
        if name.is_empty() {
            continue;
        }
        assert!(
            canonicalize_builtin_service_alias(name).is_some(),
            "server NAME {name:?} is not in BUILTIN_SERVICE_REGISTRY – \
             fix the typo or add it to the registry",
        );
    }
}

/// No two servers may share the same canonical name.
#[test]
fn builtin_server_names_are_unique() {
    use tauri_mcp_agent_lib::mcp::builtin;

    let all_names: &[&str] = &[
        builtin::planning::NAME,
        builtin::scratchpad::NAME,
        builtin::workspace::NAME,
        builtin::knowledge::NAME,
        builtin::history::NAME,
        builtin::agent::NAME,
        builtin::skills::NAME,
        builtin::playbook::NAME,
        builtin::attachments::NAME,
        builtin::ui::NAME,
        builtin::browser::NAME,
        builtin::scheduled_task::NAME,
        builtin::bootstrap::NAME,
        builtin::media::NAME,
        builtin::tool::NAME,
    ];

    let mut seen = std::collections::HashSet::new();
    for name in all_names {
        if name.is_empty() {
            continue;
        }
        assert!(seen.insert(*name), "duplicate server NAME {name:?}");
    }
}

/// BUILTIN_SERVICE_REGISTRY must not contain duplicate canonical entries.
#[test]
fn registry_has_no_duplicate_canonicals() {
    let mut seen = std::collections::HashSet::new();
    for entry in BUILTIN_SERVICE_REGISTRY {
        assert!(
            seen.insert(entry.canonical),
            "duplicate canonical {:?} in BUILTIN_SERVICE_REGISTRY",
            entry.canonical,
        );
    }
}

/// Server list and registry must stay in sync.
/// Catches: registry entry added but no server implements it (or vice-versa).
#[test]
fn registry_and_server_list_are_in_sync() {
    use tauri_mcp_agent_lib::mcp::builtin;

    for entry in BUILTIN_SERVICE_REGISTRY {
        let name = match entry.variant {
            BuiltinServiceId::Planning => builtin::planning::NAME,
            BuiltinServiceId::Scratchpad => builtin::scratchpad::NAME,
            BuiltinServiceId::Workspace => builtin::workspace::NAME,
            BuiltinServiceId::Agent => builtin::agent::NAME,
            BuiltinServiceId::Knowledge => builtin::knowledge::NAME,
            BuiltinServiceId::History => builtin::history::NAME,
            BuiltinServiceId::Skills => builtin::skills::NAME,
            BuiltinServiceId::Playbook => builtin::playbook::NAME,
            BuiltinServiceId::Attachments => builtin::attachments::NAME,
            BuiltinServiceId::Ui => builtin::ui::NAME,
            BuiltinServiceId::Browser => builtin::browser::NAME,
            BuiltinServiceId::ScheduledTask => builtin::scheduled_task::NAME,
            BuiltinServiceId::Bootstrap => builtin::bootstrap::NAME,
            BuiltinServiceId::Tool => builtin::tool::NAME,
            BuiltinServiceId::Media => builtin::media::NAME,
        };

        if name.is_empty() {
            continue;
        }

        assert_eq!(
            name, entry.canonical,
            "Server NAME constant for {:?} must match registry canonical",
            entry.variant
        );
    }
}

// ─── Assistant tool schema regression tests ──────────────────────────────────
// Regression: createAssistant and updateAssistant tool schemas were missing the
// `description` field, so AI agents had no way to set assistant descriptions
// via MCP tools.

/// Extracts the properties map from an object-type `JSONSchema`, panicking
/// with a helpful message if the schema is not an Object variant.
fn extract_object_properties(
    schema: &tauri_mcp_agent_lib::mcp::schema::JSONSchema,
    context: &str,
) -> std::collections::HashMap<String, tauri_mcp_agent_lib::mcp::schema::JSONSchema> {
    match &schema.schema_type {
        tauri_mcp_agent_lib::mcp::schema::JSONSchemaType::Object { properties, .. } => properties
            .clone()
            .unwrap_or_else(|| panic!("{context}: input_schema has no properties")),
        other => panic!("{context}: expected Object schema, got {other:?}"),
    }
}

#[test]
fn create_assistant_tool_schema_includes_description_field() {
    let tool = assistant_tools::create_assistant_tool();
    let props = extract_object_properties(&tool.input_schema, "createAssistant");
    assert!(
        props.contains_key("description"),
        "createAssistant input_schema must include a 'description' property; \
         found keys: {:?}",
        props.keys().collect::<Vec<_>>()
    );
}

#[test]
fn update_assistant_tool_schema_includes_description_field() {
    let tool = assistant_tools::update_assistant_tool();
    let props = extract_object_properties(&tool.input_schema, "updateAssistant");
    assert!(
        props.contains_key("description"),
        "updateAssistant input_schema must include a 'description' property; \
         found keys: {:?}",
        props.keys().collect::<Vec<_>>()
    );
}

#[test]
fn agent_create_tool_name_is_unprefixed() {
    let create_tool = agent_tools::all_tools()
        .into_iter()
        .find(|tool| tool.title.as_deref() == Some("Create Agent Configuration"))
        .expect("create tool must exist");

    assert_eq!(
        create_tool.name, "create",
        "builtin agent tool names must remain unprefixed; the proxy adds 'agent__'"
    );
}

#[test]
fn tool_transport_schema_allows_env_and_header_maps() {
    let register_tool = tool_tools::register_server_tool();
    let props = extract_object_properties(&register_tool.input_schema, "register");
    let transport = props
        .get("transport")
        .expect("register input_schema must contain transport");
    let transport_props = extract_object_properties(transport, "transport");

    for key in ["env", "headers"] {
        let field = transport_props
            .get(key)
            .unwrap_or_else(|| panic!("transport schema missing '{key}'"));

        match &field.schema_type {
            JSONSchemaType::Object {
                additional_properties,
                ..
            } => assert_eq!(
                *additional_properties,
                Some(true),
                "{key} must allow arbitrary key/value pairs"
            ),
            other => panic!("{key} should be an object map, got {other:?}"),
        }
    }
}

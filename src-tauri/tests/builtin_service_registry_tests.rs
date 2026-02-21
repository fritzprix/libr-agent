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
    ToolResultAcceptance, BUILTIN_SERVICE_REGISTRY, CORE_BUILTIN_SERVICE_ALIASES,
};
use tauri_mcp_agent_lib::agent::AgentConfig;
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
        allowed_built_in_service_aliases: aliases
            .map(|values| values.into_iter().map(|v| v.to_string()).collect()),
        temperature: 1.0,
        max_tokens: None,
        max_depth: None,
        max_fanout: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
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

    let message = create_tool_result_message_with_content(session_id, tool_call_id, content);

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

    let message = create_tool_result_message(session_id, tool_call_id, content_str.to_string());

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

    let message = create_error_tool_result(session_id, tool_call_id, error_msg);

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

    let message = create_tool_result_message_with_content(session_id, tool_call_id, content);

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
/// NOTE: "session_api" and "contentstore" are NOT recognised canonical names —
/// canonicalize_builtin_service_alias() returns None for them and logs a warning.
/// "swarm" and "content_store" appear in the result because they are CORE aliases
/// (always enabled), NOT because legacy name normalization occurred.
/// If legacy alias mapping is needed, add explicit entries to
/// canonicalize_builtin_service_alias() and update this test.
#[test]
fn extract_builtin_tool_ids_core_aliases_always_present_despite_unknown_inputs() {
    let config = mock_agent_config(Some(vec!["session_api", "contentstore", "browser"]));
    let tool_ids = extract_builtin_tool_ids(&config);

    // These are present because they are CORE aliases, not because of alias normalization.
    assert!(
        tool_ids.contains(&"swarm".to_string()),
        "swarm is a core alias and must always be present"
    );
    assert!(
        tool_ids.contains(&"content_store".to_string()),
        "content_store is a core alias and must always be present"
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
        assert!(
            tool_ids.contains(&alias.to_string()),
            "core alias {alias:?} must always be present"
        );
    }
    assert!(
        tool_ids.contains(&"browser".to_string()),
        "browser was explicitly requested and must be present"
    );
}

// ─── Server name / registry regression tests ─────────────────────────────────
// Original bug: ContentStoreServer::name() returned "contentstore" while the
// registry had "content_store". All four tests below would have caught it.

/// Every concrete server NAME must be a recognised canonical in the registry.
#[test]
fn each_builtin_server_name_is_in_registry() {
    use tauri_mcp_agent_lib::mcp::builtin;

    let all_names: &[&str] = &[
        builtin::planning::NAME,
        builtin::workspace::NAME,
        builtin::knowledge::NAME,
        builtin::assistant::NAME,
        builtin::skills::NAME,
        builtin::playbook::NAME,
        builtin::content_store::NAME,
        builtin::session_api::NAME,
        builtin::ui::NAME,
        builtin::browser::NAME,
        builtin::bootstrap::NAME,
        builtin::mcp_manager::NAME,
    ];

    for name in all_names {
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
        builtin::workspace::NAME,
        builtin::knowledge::NAME,
        builtin::assistant::NAME,
        builtin::skills::NAME,
        builtin::playbook::NAME,
        builtin::content_store::NAME,
        builtin::session_api::NAME,
        builtin::ui::NAME,
        builtin::browser::NAME,
        builtin::bootstrap::NAME,
        builtin::mcp_manager::NAME,
    ];

    let mut seen = std::collections::HashSet::new();
    for name in all_names {
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

    let server_names: std::collections::HashSet<&str> = [
        builtin::planning::NAME,
        builtin::workspace::NAME,
        builtin::knowledge::NAME,
        builtin::assistant::NAME,
        builtin::skills::NAME,
        builtin::playbook::NAME,
        builtin::content_store::NAME,
        builtin::session_api::NAME,
        builtin::ui::NAME,
        builtin::browser::NAME,
        builtin::bootstrap::NAME,
        builtin::mcp_manager::NAME,
    ]
    .iter()
    .copied()
    .collect();

    assert_eq!(
        server_names.len(),
        BUILTIN_SERVICE_REGISTRY.len(),
        "server list ({}) and registry ({}) diverged – update both together",
        server_names.len(),
        BUILTIN_SERVICE_REGISTRY.len(),
    );

    for entry in BUILTIN_SERVICE_REGISTRY {
        assert!(
            server_names.contains(entry.canonical),
            "registry canonical {:?} has no server NAME",
            entry.canonical,
        );
    }
}

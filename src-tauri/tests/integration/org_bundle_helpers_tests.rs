//! Org portable bundle helper coverage (via consolidated `integration_tests`).
//! Runs on Linux/macOS only — the harness is `#![cfg(not(windows))]`.

use tauri_mcp_agent_lib::commands::migration_commands::models::ORG_BUNDLE_KIND;
use tauri_mcp_agent_lib::commands::migration_commands::{
    extract_mcp_server_ids_from_config, sanitize_org_id_for_path,
};

#[test]
fn org_bundle_kind_constant() {
    assert_eq!(ORG_BUNDLE_KIND, "org_bundle");
}

#[test]
fn extract_mcp_ids_from_assistant_config() {
    let ids = extract_mcp_server_ids_from_config(
        r#"{"systemPrompt":"x","mcpServerIds":["mcp-1","mcp-2"],"disabledSkills":["s"]}"#,
    );
    assert_eq!(ids, vec!["mcp-1".to_string(), "mcp-2".to_string()]);
}

#[test]
fn extract_mcp_ids_missing_field() {
    let ids = extract_mcp_server_ids_from_config(r#"{"systemPrompt":"x"}"#);
    assert!(ids.is_empty());
}

#[test]
fn sanitize_org_id_rejects_path_traversal() {
    assert_eq!(sanitize_org_id_for_path("org-abc"), "org-abc");
    assert_eq!(sanitize_org_id_for_path("../evil"), "___evil");
    assert_eq!(sanitize_org_id_for_path(""), "unknown-org");
}

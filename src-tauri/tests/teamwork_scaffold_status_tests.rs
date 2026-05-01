pub mod common;

use std::fs;

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::inspect_teamwork_scaffold;

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should create");
    }
    fs::write(path, content).expect("file should write");
}

#[test]
fn inspect_teamwork_scaffold_reports_missing_constitution_and_recommends_teamwork() {
    common::register_sqlite_vec();
    let temp_dir = tempfile::tempdir().expect("temp dir should create");

    let status = inspect_teamwork_scaffold(temp_dir.path());

    assert_eq!(
        status.missing_files,
        vec![
            "agents.md".to_string(),
            "MISSION.md".to_string(),
            "ROLES.md".to_string(),
            "coordination/KANBAN.md".to_string(),
            "coordination/HANDOFF.md".to_string(),
        ]
    );
    assert!(!status.manifest_present);
    assert_eq!(status.recommended_skill.as_deref(), Some("teamwork"));
    assert!(
        !status.is_ready_for_explicit_org(),
        "missing scaffold must not be treated as org-ready"
    );
}

#[test]
fn inspect_teamwork_scaffold_accepts_complete_org_workspace() {
    common::register_sqlite_vec();
    let temp_dir = tempfile::tempdir().expect("temp dir should create");
    let workspace = temp_dir.path();

    write_file(&workspace.join("agents.md"), "# agents");
    write_file(&workspace.join("MISSION.md"), "# mission");
    write_file(&workspace.join("ROLES.md"), "# roles");
    write_file(
        &workspace.join("coordination").join("KANBAN.md"),
        "# kanban",
    );
    write_file(
        &workspace.join("coordination").join("HANDOFF.md"),
        "# handoff",
    );
    write_file(
        &workspace.join(".libragent").join("teamwork.json"),
        &serde_json::to_string_pretty(&json!({
            "executionSubstrate": {
                "mode": "org",
                "orgLineage": { "intended": true }
            }
        }))
        .expect("manifest should serialize"),
    );

    let status = inspect_teamwork_scaffold(workspace);

    assert!(status.missing_files.is_empty());
    assert!(status.manifest_present);
    assert_eq!(status.execution_substrate_mode.as_deref(), Some("org"));
    assert_eq!(status.org_lineage_intended, Some(true));
    assert_eq!(status.recommended_skill, None);
    assert!(
        status.is_ready_for_explicit_org(),
        "complete org scaffold should be treated as ready"
    );
}

#[test]
fn inspect_teamwork_scaffold_flags_manifest_substrate_mismatch() {
    common::register_sqlite_vec();
    let temp_dir = tempfile::tempdir().expect("temp dir should create");
    let workspace = temp_dir.path();

    write_file(&workspace.join("agents.md"), "# agents");
    write_file(&workspace.join("MISSION.md"), "# mission");
    write_file(&workspace.join("ROLES.md"), "# roles");
    write_file(
        &workspace.join("coordination").join("KANBAN.md"),
        "# kanban",
    );
    write_file(
        &workspace.join("coordination").join("HANDOFF.md"),
        "# handoff",
    );
    write_file(
        &workspace.join(".libragent").join("teamwork.json"),
        &serde_json::to_string_pretty(&json!({
            "executionSubstrate": {
                "mode": "plain-child-sessions",
                "orgLineage": { "intended": false }
            }
        }))
        .expect("manifest should serialize"),
    );

    let status = inspect_teamwork_scaffold(workspace);

    assert!(status.missing_files.is_empty());
    assert_eq!(
        status.execution_substrate_mode.as_deref(),
        Some("plain-child-sessions")
    );
    assert_eq!(status.org_lineage_intended, Some(false));
    assert_eq!(status.recommended_skill.as_deref(), Some("teamwork"));
    assert!(
        !status.is_ready_for_explicit_org(),
        "non-org manifest should not be treated as org-ready"
    );
}

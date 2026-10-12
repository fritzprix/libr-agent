//! Windows-safe unit tests for createOrg teamwork.json orgLineage stamping.
//! (Not behind cfg(not(windows)) — pure JSON helper, no Tauri WebView link.)

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    inspect_teamwork_scaffold, stamp_org_identity_into_manifest,
};

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent dir should create");
    }
    std::fs::write(path, content).expect("file should write");
}

fn write_complete_org_scaffold(workspace: &std::path::Path, manifest: &serde_json::Value) {
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
        &workspace.join("coordination").join("DECISIONS.md"),
        "# decisions",
    );
    write_file(&workspace.join("coordination").join("RISKS.md"), "# risks");
    write_file(
        &workspace.join("coordination").join("DISCUSSION.md"),
        "# discussion",
    );
    write_file(
        &workspace.join(".libragent").join("teamwork.json"),
        &serde_json::to_string_pretty(manifest).expect("manifest should serialize"),
    );
}

#[test]
fn stamp_org_identity_seeds_nested_org_lineage_from_top_level_alias() {
    let mut manifest = json!({
        "executionSubstrate": {
            "mode": "org"
        },
        "orgLineage": { "intended": true }
    });

    assert!(stamp_org_identity_into_manifest(
        &mut manifest,
        "org-abc",
        "Harbor Org",
        "root-session-1",
    ));

    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["intended"],
        json!(true)
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["orgId"],
        json!("org-abc")
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["orgName"],
        json!("Harbor Org")
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["rootSessionId"],
        json!("root-session-1")
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["rootAction"],
        json!("agent__createOrg")
    );
    assert_eq!(manifest["orgLineage"]["orgId"], json!("org-abc"));
    assert_eq!(
        manifest["orgLineage"]["rootSessionId"],
        json!("root-session-1")
    );
}

#[test]
fn stamp_org_identity_updates_existing_nested_org_lineage() {
    let mut manifest = json!({
        "executionSubstrate": {
            "mode": "org",
            "orgLineage": {
                "intended": true,
                "childAction": "agent__spawnSession"
            }
        }
    });

    assert!(stamp_org_identity_into_manifest(
        &mut manifest,
        "org-xyz",
        "Nested Org",
        "root-session-2",
    ));

    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["orgId"],
        json!("org-xyz")
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["childAction"],
        json!("agent__spawnSession"),
        "existing nested fields must be preserved"
    );
    assert!(
        manifest.get("orgLineage").is_none(),
        "must not invent a top-level orgLineage when absent"
    );
}

#[test]
fn inspect_accepts_top_level_alias_and_stamp_makes_create_org_durable() {
    let temp_dir = tempfile::tempdir().expect("temp dir should create");
    let workspace = temp_dir.path();
    write_complete_org_scaffold(
        workspace,
        &json!({
            "executionSubstrate": { "mode": "org" },
            "orgLineage": { "intended": true }
        }),
    );

    let status = inspect_teamwork_scaffold(workspace);
    assert!(
        status.is_ready_for_explicit_org(),
        "sibling alias must pass createOrg preflight"
    );

    let manifest_path = workspace.join(".libragent").join("teamwork.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    assert!(stamp_org_identity_into_manifest(
        &mut manifest,
        "org-harbor",
        "Collab",
        "sess-root",
    ));
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize"),
    )
    .expect("write stamped manifest");

    let stamped: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("reread"))
            .expect("parse stamped");
    assert_eq!(
        stamped["executionSubstrate"]["orgLineage"]["rootSessionId"],
        json!("sess-root")
    );
    assert_eq!(stamped["orgLineage"]["orgId"], json!("org-harbor"));
}

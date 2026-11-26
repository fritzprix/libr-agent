//! #2927 D2/D6 production-path coverage added for gatekeeper round 1
//! (#2954 G1.4, G1.5): delete rejection, scan population, update-target
//! resolution, and the assistant-copy marker strip all run through the real
//! functions.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri_mcp_agent_lib::services::hooks::registry::{
    get_session_plugins_directory, get_user_plugins_directory, save_source_in,
    upsert_session_store_entry, PluginSourceManifest,
};
use tauri_mcp_agent_lib::services::skill_service::{
    copy_global_to_assistant, delete_assistant_skill, delete_user_skill,
    get_assistant_skills_directory, get_user_skills_directory, invalidate_skill_scan_cache,
    resolve_plugin_by_target, scan_skills_directory, skill_storage_directory_name, PluginScope,
    PLUGIN_SKILL_OWNER_FILE, SKILL_FILE_NAME,
};
use uuid::Uuid;

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

/// A valid plugin source id for `save_source_in` (#2927): `plugin-` plus 16
/// lowercase hex characters, 23 in total.
fn valid_source_id() -> String {
    let hex: String = Uuid::new_v4().simple().to_string()[..16].to_string();
    format!("plugin-{hex}")
}

fn create_skill_dir(root: &Path, skill_name: &str) -> PathBuf {
    let storage_name = skill_storage_directory_name(skill_name).expect("storage name");
    let skill_dir = root.join(storage_name);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join(SKILL_FILE_NAME),
        format!("---\nname: {skill_name}\ndescription: test skill\n---\n# {skill_name}\n"),
    )
    .expect("write skill file");
    skill_dir
}

// G1.5: `delete_user_skill` must reject a plugin-owned skill and name the owner,
// leaving the skill in place (#2927 D2 read-only rule).
#[tokio::test]
async fn delete_user_skill_rejects_plugin_owned_skill_and_names_the_owner() {
    let user_dir = get_user_skills_directory().expect("user dir");
    let name = unique("owned-skill");
    let owner_id = unique("src");
    let skill_dir = create_skill_dir(&user_dir, &name);
    fs::write(skill_dir.join(PLUGIN_SKILL_OWNER_FILE), &owner_id).expect("write marker");

    let err = delete_user_skill(name.clone())
        .await
        .expect_err("a plugin-owned skill must not be deletable on its own");
    assert!(err.contains(&owner_id), "error names the owner: {err}");
    assert!(
        err.contains("uninstall"),
        "error tells how to remove it: {err}"
    );
    assert!(
        skill_dir.exists(),
        "the rejected delete must leave the skill in place"
    );

    fs::remove_dir_all(&skill_dir).expect("cleanup");
}

// G1.5: the scan populates `plugin_owner` from the marker; unmarked skills
// stay `None` (#2927 D2).
#[tokio::test]
async fn scan_skills_directory_populates_plugin_owner() {
    let root = tempfile::tempdir().expect("tempdir");
    let owned = create_skill_dir(root.path(), "owned-skill");
    fs::write(owned.join(PLUGIN_SKILL_OWNER_FILE), "src-42").expect("write marker");
    create_skill_dir(root.path(), "plain-skill");

    let skills = scan_skills_directory(root.path()).await.expect("scan");
    let owned = skills
        .iter()
        .find(|skill| skill.name == "owned-skill")
        .expect("owned skill scanned");
    assert_eq!(owned.plugin_owner.as_deref(), Some("src-42"));
    let plain = skills
        .iter()
        .find(|skill| skill.name == "plain-skill")
        .expect("plain skill scanned");
    assert_eq!(plain.plugin_owner, None);
}

fn manifest(source_url: &str, source_id: &str) -> PluginSourceManifest {
    PluginSourceManifest {
        source_url: source_url.to_string(),
        source_id: source_id.to_string(),
        skill_names: vec![],
        hooks: None,
        hooks_approved: false,
        approval_hashes: BTreeMap::new(),
        created_at: 0,
        plugin: None,
        hook_files: vec![],
        mcp_servers: vec![],
        mcp_approved: false,
        enabled: true,
        installed_commit: None,
        inline_hooks: vec![],
    }
}

// G1.5: `resolve_plugin_by_target` matches session installs first, then
// global, and errors on empty, unknown, and ambiguous targets (#2927 item 4).
#[tokio::test]
async fn resolve_plugin_by_target_prefers_session_then_global_and_rejects_bad_targets() {
    let session_id = unique("plugin-owner-session");
    let session_dir = get_session_plugins_directory(&session_id).expect("session plugins dir");
    let global_dir = get_user_plugins_directory().expect("global plugins dir");
    fs::create_dir_all(&global_dir).expect("global plugins dir");
    // A unique repo name keeps this test isolated from real installs and
    // earlier test runs that share the actual global plugins directory.
    let repo = format!("update-target-{}", Uuid::new_v4().simple());
    let url = format!("https://github.com/acme/{repo}");
    let sess_id = valid_source_id();
    let glob_id = valid_source_id();
    let glob_dup_id = valid_source_id();

    save_source_in(&session_dir, manifest(&url, &sess_id)).expect("save session record");
    // A real session install also records the plugin in the store manifest;
    // `load_session_plugins` resolves entries through it (#2942 D1).
    upsert_session_store_entry(
        &session_dir,
        tauri_mcp_agent_lib::services::hooks::registry::SessionPluginStoreEntry {
            source_id: sess_id.clone(),
            source_url: url.to_string(),
            installed_at: 0,
        },
    )
    .expect("upsert store entry");
    save_source_in(&global_dir, manifest(&url, &glob_id)).expect("save global record");

    let entry = resolve_plugin_by_target(&session_id, &repo).expect("resolve session");
    assert_eq!(entry.scope, PluginScope::Session);
    assert_eq!(entry.id, sess_id);
    assert_eq!(entry.source_url, url);

    // Without the session record the same target resolves to the global one.
    fs::remove_dir_all(session_dir.join(&sess_id)).expect("remove session record");
    let entry = resolve_plugin_by_target(&session_id, &repo).expect("resolve global");
    assert_eq!(entry.scope, PluginScope::Global);
    assert_eq!(entry.id, glob_id);

    // A second global record with the same URL makes the URL target ambiguous.
    save_source_in(&global_dir, manifest(&url, &glob_dup_id)).expect("save duplicate");
    let err =
        resolve_plugin_by_target(&session_id, &url).expect_err("two matches must be ambiguous");
    assert!(err.contains("ambiguous"), "error says ambiguous: {err}");
    fs::remove_dir_all(global_dir.join(&glob_dup_id)).expect("remove duplicate");

    // Unknown and empty targets error.
    assert!(resolve_plugin_by_target(&session_id, "definitely-not-installed").is_err());
    assert!(resolve_plugin_by_target(&session_id, "   ").is_err());

    fs::remove_dir_all(global_dir.join(&glob_id)).expect("cleanup global record");
}

// G1.4: an assistant copy is the user's own skill — the plugin-owner marker
// must not travel with it, and a failed strip must not be swallowed.
#[tokio::test]
async fn copy_global_to_assistant_drops_plugin_owner_marker() {
    let user_dir = get_user_skills_directory().expect("user dir");
    let name = unique("assistant-copy-skill");
    let owner_id = unique("src");
    let skill_dir = create_skill_dir(&user_dir, &name);
    fs::write(skill_dir.join(PLUGIN_SKILL_OWNER_FILE), &owner_id).expect("write marker");
    invalidate_skill_scan_cache();

    let assistant_id = Uuid::new_v4().to_string();
    copy_global_to_assistant(assistant_id.clone(), name.clone())
        .await
        .expect("copy to assistant");

    let assistant_dir = get_assistant_skills_directory(&assistant_id).expect("assistant dir");
    let copied = assistant_dir.join(skill_storage_directory_name(&name).expect("storage name"));
    assert!(
        copied.join(SKILL_FILE_NAME).is_file(),
        "skill itself is copied"
    );
    assert!(
        !copied.join(PLUGIN_SKILL_OWNER_FILE).exists(),
        "the assistant copy must not inherit the plugin-owner marker"
    );

    delete_assistant_skill(assistant_id, name.clone())
        .await
        .expect("remove assistant copy");
    fs::remove_dir_all(&skill_dir).expect("cleanup user skill");
    invalidate_skill_scan_cache();
}

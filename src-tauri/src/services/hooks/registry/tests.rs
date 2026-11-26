use super::super::schema::{HookEvent, HookHandler};
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

/// #2921 (gatekeeper G1.2): `.claude-plugin/plugin.json` metadata must
/// survive `save_source_in`/`get_source_in` so the Skills view can show
/// plugin name/version/description across restarts.
#[test]
fn plugin_metadata_roundtrips_through_source_store() {
    let base = tempfile::tempdir().unwrap();
    let source_id = format!("plugin-{}", "0".repeat(16));
    // The persisted hook_files list is validated against disk at save,
    // so lay down the file it names.
    let source_root = base.path().join(&source_id);
    fs::create_dir_all(source_root.join("hooks")).unwrap();
    fs::write(
        source_root.join("hooks").join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo hi"}]}}"#,
    )
    .unwrap();
    let record = save_source_in(
        base.path(),
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "https://example.com/p".to_string(),
            source_id: source_id.clone(),
            skill_names: vec!["alpha".to_string()],
            hooks: None,
            hooks_approved: false,
            approval_hashes: BTreeMap::new(),
            created_at: 0,
            hook_files: vec!["./hooks/hooks.json".to_string()],
            inline_hooks: vec![],
            plugin: Some(PluginMetadata {
                name: "superpowers".to_string(),
                version: Some("1.2.3".to_string()),
                description: Some("Skills for agents".to_string()),
                author: Some("Jesse Vincent".to_string()),
                homepage: None,
                repository: None,
                license: None,
                keywords: vec!["skills".to_string()],
            }),
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
    assert!(record.manifest.plugin.is_some());

    let loaded = get_source_in(base.path(), &source_id).unwrap();
    let plugin = loaded
        .manifest
        .plugin
        .as_ref()
        .expect("plugin metadata persisted");
    assert_eq!(plugin.name, "superpowers");
    assert_eq!(plugin.version.as_deref(), Some("1.2.3"));
    assert_eq!(plugin.description.as_deref(), Some("Skills for agents"));
    assert_eq!(plugin.author.as_deref(), Some("Jesse Vincent"));
    assert_eq!(plugin.keywords, vec!["skills".to_string()]);
    assert_eq!(loaded.manifest.skill_names, vec!["alpha".to_string()]);
    assert_eq!(
        loaded.manifest.hook_files,
        vec!["./hooks/hooks.json".to_string()],
    );

    let cfg = hook_config_for_record(&loaded).unwrap();
    assert!(cfg.events.contains_key(&HookEvent::SessionStart));
}

/// #2927: a disabled plugin keeps its files and approval record, but the
/// hook loaders skip it, and the flag round-trips through the store.
#[test]
fn disabled_source_is_skipped_by_hook_loaders() {
    let base = tempfile::tempdir().unwrap();
    let source_id = format!("plugin-{}", "1".repeat(16));
    save_source_in(base.path(), test_manifest(&source_id, true)).unwrap();
    assert_eq!(list_approved_sources_in(base.path()).len(), 1);

    let disabled = set_source_enabled_in(base.path(), &source_id, false).unwrap();
    assert!(!disabled.manifest.enabled);
    assert!(
        list_approved_sources_in(base.path()).is_empty(),
        "a disabled plugin must contribute no hooks"
    );

    let still_stored = get_source_in(base.path(), &source_id).unwrap();
    assert!(!still_stored.manifest.enabled);
    assert!(
        still_stored.manifest.hooks_approved,
        "disabling keeps the one-time approval record"
    );
    assert!(
        still_stored.root_dir.is_dir(),
        "disabling keeps the installed files"
    );

    let enabled_again = set_source_enabled_in(base.path(), &source_id, true).unwrap();
    assert!(enabled_again.manifest.enabled);
    assert_eq!(list_approved_sources_in(base.path()).len(), 1);
}

/// #2927: a store recorded before the flag existed has no `enabled` key
/// and must load enabled, so an upgrade never silently disables a plugin.
#[test]
fn legacy_manifest_without_enabled_field_loads_enabled() {
    let base = tempfile::tempdir().unwrap();
    let source_id = format!("plugin-{}", "2".repeat(16));
    let root = base.path().join(&source_id);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(MANIFEST_FILE),
        format!(
            r#"{{"source_url":"https://example.com/p","source_id":"{source_id}",
                "skill_names":["alpha"],"hooks":null,"hooks_approved":true,
                "approval_hashes":{{}},"created_at":0}}"#
        ),
    )
    .unwrap();

    let record = get_source_in(base.path(), &source_id).unwrap();
    assert!(
        record.manifest.enabled,
        "pre-#2927 stores must load enabled"
    );
    assert_eq!(record.manifest.installed_commit, None);
    assert_eq!(list_approved_sources_in(base.path()).len(), 1);
}

/// #2942 gatekeeper G1.4 fixtures: a manifest with optional inline
/// approval, without touching the real install pipeline. `mcp_approved`
/// mirrors the D3 install gate (approval covers MCP servers too,
/// #2942 S3.2).
fn test_manifest(source_id: &str, approved: bool) -> PluginSourceManifest {
    PluginSourceManifest {
        source_url: "https://example.com/p".to_string(),
        source_id: source_id.to_string(),
        skill_names: vec![],
        hooks: None,
        hooks_approved: approved,
        approval_hashes: if approved {
            BTreeMap::from([("SessionStart:0".to_string(), "deadbeef".to_string())])
        } else {
            BTreeMap::new()
        },
        created_at: 0,
        plugin: None,
        hook_files: vec![],
        inline_hooks: vec![],
        mcp_servers: if approved {
            vec![PluginMcpServerSnapshot {
                name: "srv".to_string(),
                entry: serde_json::json!({ "command": "bin/srv" }),
            }]
        } else {
            vec![]
        },
        mcp_approved: approved,
        enabled: true,
        installed_commit: None,
    }
}

fn entry(source_id: &str) -> SessionPluginStoreEntry {
    SessionPluginStoreEntry {
        source_id: source_id.to_string(),
        source_url: "https://example.com/p".to_string(),
        installed_at: 0,
    }
}

#[test]
fn session_store_dir_is_under_workspace_and_isolated_per_session() {
    // #2942 gatekeeper G1.4: the store lives inside the session
    // workspace at `.libragent/plugins`, and two workspaces never share
    // a store.
    let ws_a = tempfile::tempdir().unwrap();
    let ws_b = tempfile::tempdir().unwrap();
    let store_a = session_store_dir_in(ws_a.path());
    assert_eq!(
        store_a,
        ws_a.path()
            .join(".libragent")
            .join(SESSION_WORKSPACE_PLUGINS_DIR)
    );
    let store_b = session_store_dir_in(ws_b.path());
    assert!(!store_a.starts_with(&store_b) && !store_b.starts_with(&store_a));

    let id = format!("plugin-{}", "a".repeat(16));
    save_source_in(&store_a, test_manifest(&id, false)).unwrap();
    upsert_session_store_entry(&store_a, entry(&id)).unwrap();
    assert_eq!(list_session_store_sources(None, &store_a).len(), 1);
    assert!(list_session_store_sources(None, &store_b).is_empty());
}

#[test]
fn move_session_approval_to_trust_roundtrips_and_overlays() {
    // #2942 gatekeeper G1.1: after the move, the workspace manifest
    // holds no approval and only the trust record can approve; loaders
    // without the trust dir (or with a missing one) stay unapproved.
    let base = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let trust_dir = trust_root.path().join("s1");
    let id = format!("plugin-{}", "b".repeat(16));
    save_source_in(base.path(), test_manifest(&id, true)).unwrap();
    upsert_session_store_entry(base.path(), entry(&id)).unwrap();

    let before = get_source_in(base.path(), &id).unwrap();
    assert!(before.manifest.hooks_approved);
    assert!(before.manifest.mcp_approved);

    move_session_approval_to_trust(&trust_dir, base.path(), &id).unwrap();

    let scrubbed = get_source_in(base.path(), &id).unwrap();
    assert!(!scrubbed.manifest.hooks_approved);
    assert!(scrubbed.manifest.approval_hashes.is_empty());
    assert!(!scrubbed.manifest.mcp_approved);
    let trust_record = read_session_plugin_trust(&trust_dir, &id);
    assert!(trust_record.hooks_approved);
    assert!(trust_record.mcp_approved);
    assert_eq!(
        trust_record.approval_hashes,
        before.manifest.approval_hashes
    );

    let with_trust = list_session_store_sources(Some(&trust_dir), base.path());
    assert_eq!(with_trust.len(), 1);
    assert!(with_trust[0].manifest.hooks_approved);
    assert!(with_trust[0].manifest.mcp_approved);
    assert_eq!(
        with_trust[0].manifest.approval_hashes,
        before.manifest.approval_hashes
    );
    assert!(
        !list_session_store_sources(None, base.path())[0]
            .manifest
            .hooks_approved
    );
    assert!(
        !list_session_store_sources(None, base.path())[0]
            .manifest
            .mcp_approved
    );
    let other_trust = trust_root.path().join("s2");
    assert!(
        !list_session_store_sources(Some(&other_trust), base.path())[0]
            .manifest
            .hooks_approved
    );
    assert!(
        !list_session_store_sources(Some(&other_trust), base.path())[0]
            .manifest
            .mcp_approved
    );

    // Uninstall removes the trust record; a missing file reads as
    // unapproved (fail-safe).
    remove_session_plugin_trust(&trust_dir, &id).unwrap();
    assert!(!read_session_plugin_trust(&trust_dir, &id).hooks_approved);
    assert!(!read_session_plugin_trust(&trust_dir, &id).mcp_approved);
    remove_session_plugin_trust(&trust_dir, &id).unwrap();
}

#[test]
fn tampered_workspace_mcp_approved_flag_is_ignored() {
    // #2942 S3.2: the session store now lives in the agent-writable
    // workspace, so flipping mcp_approved in the workspace manifest
    // (without touching {data_dir}) must never approve a server.
    let base = tempfile::tempdir().unwrap();
    let trust_dir = tempfile::tempdir().unwrap().path().join("s1");
    let id = format!("plugin-{}", "c".repeat(16));
    save_source_in(base.path(), test_manifest(&id, false)).unwrap();
    upsert_session_store_entry(base.path(), entry(&id)).unwrap();

    // Tamper: rewrite the workspace manifest as if the user had approved.
    let manifest_path = base.path().join(&id).join(MANIFEST_FILE);
    let raw = fs::read_to_string(&manifest_path).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    json["mcp_approved"] = serde_json::Value::Bool(true);
    fs::write(&manifest_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    // Load with the (empty) trust dir and without one: stays unapproved.
    let records = list_session_store_sources(Some(&trust_dir), base.path());
    assert_eq!(records.len(), 1);
    assert!(!records[0].manifest.mcp_approved);
    assert!(
        !list_session_store_sources(None, base.path())[0]
            .manifest
            .mcp_approved
    );
}

#[test]
fn tampered_workspace_enabled_flag_is_ignored() {
    // #2927 gatekeeper G1.1: the enabled state of a session plugin lives
    // in the trust record, so a workspace manifest edited to
    // `"enabled": true` must not turn a plugin the user disabled back on.
    let base = tempfile::tempdir().unwrap();
    let trust_dir = tempfile::tempdir().unwrap().path().join("s1");
    let id = format!("plugin-{}", "d".repeat(16));
    save_source_in(base.path(), test_manifest(&id, true)).unwrap();
    upsert_session_store_entry(base.path(), entry(&id)).unwrap();
    move_session_approval_to_trust(&trust_dir, base.path(), &id).unwrap();

    let disabled = set_session_source_enabled_in(&trust_dir, base.path(), &id, false).unwrap();
    assert!(!disabled.manifest.enabled);
    assert!(
        !read_session_plugin_trust(&trust_dir, &id).enabled,
        "the trust record is where the state is kept"
    );

    // Tamper: rewrite the workspace manifest as if the user had enabled
    // it again.
    let manifest_path = base.path().join(&id).join(MANIFEST_FILE);
    let raw = fs::read_to_string(&manifest_path).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    json["enabled"] = serde_json::Value::Bool(true);
    fs::write(&manifest_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let records = list_session_store_sources(Some(&trust_dir), base.path());
    assert_eq!(records.len(), 1);
    assert!(
        !records[0].manifest.enabled,
        "a workspace edit must not re-enable a disabled plugin"
    );
    assert!(
        records[0].manifest.hooks_approved,
        "the approval overlay still comes from the trust record"
    );
    // ... so its approved hooks still do not run.
    let no_global = tempfile::tempdir().unwrap();
    assert!(
        list_effective_approved_sources_in(no_global.path(), Some(base.path()), Some(&trust_dir))
            .is_empty(),
        "a disabled plugin contributes no hooks"
    );

    // Only the trust record enables it again.
    set_session_source_enabled_in(&trust_dir, base.path(), &id, true).unwrap();
    assert!(
        list_session_store_sources(Some(&trust_dir), base.path())[0]
            .manifest
            .enabled
    );
}

#[test]
fn session_plugin_without_trust_record_stays_enabled() {
    // #2927: a plugin installed before the trust record carried `enabled`
    // (or with no trust store at all) must load enabled — the state it
    // had before this field existed — while remaining unapproved.
    let base = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let id = format!("plugin-{}", "e".repeat(16));
    save_source_in(base.path(), test_manifest(&id, true)).unwrap();
    upsert_session_store_entry(base.path(), entry(&id)).unwrap();

    let record = list_session_store_sources(None, base.path());
    assert_eq!(record.len(), 1);
    assert!(record[0].manifest.enabled);
    assert!(!record[0].manifest.hooks_approved);

    let empty_trust = trust_root.path().join("s1");
    assert!(
        list_session_store_sources(Some(&empty_trust), base.path())[0]
            .manifest
            .enabled,
        "a missing trust file must not disable a plugin"
    );
}

/// Inline manifest hooks (#3005): a plugin with no `hooks/` directory still
/// dispatches the hooks its manifest declares, and the stored JSON is what
/// dispatch reads.
#[test]
fn inline_manifest_hooks_load_for_a_plugin_without_a_hooks_dir() {
    let base = tempfile::tempdir().unwrap();
    let source_id = format!("plugin-{}", "2".repeat(16));
    let root_dir = base.path().join(&source_id);
    fs::create_dir_all(&root_dir).unwrap();
    let manifest = PluginSourceManifest {
        source_url: "test://local".to_string(),
        source_id: source_id.clone(),
        skill_names: vec![],
        hooks: None,
        hooks_approved: true,
        approval_hashes: BTreeMap::new(),
        created_at: 0,
        plugin: None,
        hook_files: vec![],
        inline_hooks: vec![serde_json::json!({
            "PreToolUse": [{ "matcher": "Bash", "command": "echo inline" }],
            "SessionEnd": [{ "command": "echo bye" }]
        })],
        mcp_servers: Vec::new(),
        mcp_approved: false,
        enabled: true,
        installed_commit: None,
    };
    let record = save_source_in(base.path(), manifest).unwrap();
    assert!(
        record.manifest.hooks.as_ref().is_some_and(|h| h.validated),
        "inline hooks must be validated at save time"
    );

    let cfg = hook_config_for_record(&record).unwrap();
    assert_eq!(
        cfg.events[&HookEvent::PreToolUse][0].handlers[0].command,
        "echo inline"
    );
    assert!(cfg.events.contains_key(&HookEvent::SessionEnd));
}

/// A record whose inline hooks are invalid fails the save instead of
/// installing a plugin that would dispatch nothing (#3005).
#[test]
fn invalid_inline_manifest_hooks_fail_the_save() {
    let base = tempfile::tempdir().unwrap();
    let source_id = format!("plugin-{}", "3".repeat(16));
    fs::create_dir_all(base.path().join(&source_id)).unwrap();
    let manifest = PluginSourceManifest {
        source_url: "test://local".to_string(),
        source_id,
        skill_names: vec![],
        hooks: None,
        hooks_approved: true,
        approval_hashes: BTreeMap::new(),
        created_at: 0,
        plugin: None,
        hook_files: vec![],
        inline_hooks: vec![serde_json::json!({ "PreToolUse": "not-an-array" })],
        mcp_servers: Vec::new(),
        mcp_approved: false,
        enabled: true,
        installed_commit: None,
    };
    let error = save_source_in(base.path(), manifest).unwrap_err();
    assert!(error.contains("PreToolUse"), "{error}");
}

fn exec_handler(command: &str, args: Option<Vec<String>>) -> HookHandler {
    HookHandler {
        command: command.to_string(),
        args,
        powershell: None,
        timeout_ms: crate::services::hooks::schema::default_hook_timeout_ms(HookEvent::PreToolUse),
        r#async: false,
        shell: None,
        http: None,
    }
}

/// #3005 gatekeeper G1.1: in exec form the script usually lives in
/// `args`, so its content must be part of the approval hash — editing it
/// after approval invalidates the handler.
#[test]
fn exec_form_args_script_edit_invalidates_the_approval_hash() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("x.js");
    fs::write(&script, "console.log('v1')").unwrap();
    let handler = exec_handler("node", Some(vec!["${CLAUDE_PLUGIN_ROOT}/x.js".to_string()]));

    let before = approval_hash_value(root.path(), &handler);
    fs::write(&script, "console.log('evil')").unwrap();
    let after = approval_hash_value(root.path(), &handler);

    assert_ne!(
        before, after,
        "editing a script referenced by exec-form args must change the approval hash"
    );
}

/// #3005 gatekeeper G2.2: an exec-form argument can wrap a whole command
/// string (`bash -c "./hooks/x.sh --fix"`), where no argument is a file
/// path on its own. The path tokens inside each argument must be hashed
/// too, or x.sh can be edited after approval and still run.
#[test]
fn exec_form_command_string_path_tokens_are_hashed() {
    let root = tempfile::tempdir().unwrap();
    let hooks = root.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let script = hooks.join("x.sh");
    fs::write(&script, "#!/bin/sh\necho v1").unwrap();
    let handler = exec_handler(
        "bash",
        Some(vec!["-c".to_string(), "./hooks/x.sh --fix".to_string()]),
    );

    let before = approval_hash_value(root.path(), &handler);
    fs::write(&script, "#!/bin/sh\necho evil").unwrap();
    let after = approval_hash_value(root.path(), &handler);

    assert_ne!(
        before, after,
        "editing a script wrapped inside an exec-form argument must change the approval hash"
    );
}

/// #3005 gatekeeper G1.3: two handlers with the same event, matcher and
/// command but different `args` get different approval keys, while a
/// handler without `args` keeps the key earlier installs recorded.
#[test]
fn approval_hash_key_separates_handlers_by_args() {
    let first = exec_handler("node", Some(vec!["a.js".to_string()]));
    let second = exec_handler("node", Some(vec!["b.js".to_string()]));
    let plain = exec_handler("node", None);

    assert_ne!(
        approval_hash_key(HookEvent::PreToolUse, None, &first),
        approval_hash_key(HookEvent::PreToolUse, None, &second),
        "same command with different argv must not share one approval key"
    );

    // Keys of handlers without `args` are unchanged: the digest input of
    // the pre-#3005 formula.
    let expected_legacy = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}|*|{}|{}",
                HookEvent::PreToolUse.as_str(),
                plain.command,
                ""
            )
            .as_bytes(),
        )
    );
    assert_eq!(
        approval_hash_key(HookEvent::PreToolUse, None, &plain),
        expected_legacy,
        "a handler without args must keep the key an earlier install recorded"
    );
}

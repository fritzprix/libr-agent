use super::*;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use std::fs;
use std::path::Path;

/// Write a session-store source record the way `save_source_in` does:
/// `<base>/<source_id>/manifest.json` with a matching `source_id`.
fn write_store_manifest(
    base: &Path,
    source_id: &str,
    mcp_approved: bool,
    servers_json: &str,
) -> std::path::PathBuf {
    let root = base.join(source_id);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("manifest.json"),
        format!(
            r#"{{
              "source_url": "https://example.com/{source_id}",
              "source_id": "{source_id}",
              "skill_names": [],
              "hooks": null,
              "hooks_approved": false,
              "approval_hashes": {{}},
              "created_at": 0,
              "plugin": null,
              "hook_files": [],
              "mcp_servers": {servers_json},
              "mcp_approved": {mcp_approved},
              "enabled": true,
              "installed_commit": null
            }}"#
        ),
    )
    .unwrap();
    root
}

// #2924 (gatekeeper G1.4): session load reads only approved servers from
// the session store, resolves `${CLAUDE_PLUGIN_ROOT}` against the
// installed root, and keys entries with the plugin pseudo-id. Approval
// comes only from the trust record (#2942 S3.2), so both manifests carry
// the scrubbed (false) flag like real session installs do.
#[tokio::test]
async fn session_plugin_configs_are_approved_only_and_root_resolved() {
    let store = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let trust_dir = trust_root.path().join("sess-1");
    let approved_root = write_store_manifest(
        store.path(),
        "plugin-aaaa000000000000",
        false,
        r#"[{"name":"fetch","entry":{"command":"${CLAUDE_PLUGIN_ROOT}/bin/fetch","args":["--root","${CLAUDE_PLUGIN_ROOT}"]}}]"#,
    );
    write_store_manifest(
        store.path(),
        "plugin-bbbb000000000000",
        false,
        r#"[{"name":"secret","entry":{"command":"never-run"}}]"#,
    );
    add_store_entry(store.path(), "plugin-aaaa000000000000");
    add_store_entry(store.path(), "plugin-bbbb000000000000");
    record_mcp_trust(&trust_dir, "plugin-aaaa000000000000", true);

    let mut taken = std::collections::HashSet::new();
    let configs =
        build_session_plugin_server_configs(store.path(), "sess-1", Some(&trust_dir), &mut taken)
            .await;
    assert_eq!(configs.len(), 1, "unapproved source servers are skipped");
    let server = &configs[0];
    assert_eq!(server.server_name, "fetch");
    assert_eq!(server.pseudo_id, "plugin:plugin-aaaa000000000000:fetch");
    let root = approved_root.to_string_lossy();
    match &server.config.transport {
        crate::mcp::types::TransportConfig::Stdio { command, args, .. } => {
            assert_eq!(command, &format!("{root}/bin/fetch"));
            assert_eq!(args, &vec![format!("--root"), root.to_string()]);
        }
        _ => panic!("expected stdio transport"),
    }
    // The store shape keeps the `pluginSource` marker (covered by
    // `entry_converts_to_stored_config_json`); the in-memory session
    // config identifies its owner through the pseudo-id instead.
    assert!(server
        .pseudo_id
        .starts_with("plugin:plugin-aaaa000000000000:"));
    // The session key is claimed so DB-loaded servers cannot collide.
    assert!(taken.contains("fetch"));
}

/// #2942 S3.2 fixture: list a plugin in the store manifest (session
/// load is manifest-driven, #2942 D1) so the loader can see it.
fn add_store_entry(base: &Path, source_id: &str) {
    crate::services::hooks::registry::upsert_session_store_entry(
        base,
        crate::services::hooks::registry::SessionPluginStoreEntry {
            source_id: source_id.to_string(),
            source_url: format!("https://example.com/{source_id}"),
            installed_at: 0,
        },
    )
    .unwrap();
}

/// #2942 S3.2 fixture: write the out-of-workspace trust record that is
/// the only authority for MCP-server approval.
fn record_mcp_trust(trust_dir: &Path, source_id: &str, approved: bool) {
    crate::services::hooks::registry::record_session_plugin_trust(
        trust_dir,
        source_id,
        &crate::services::hooks::registry::SessionPluginTrust {
            mcp_approved: approved,
            ..Default::default()
        },
    )
    .unwrap();
}

/// #2927 fixture: turn one session plugin off the way `/plugin disable`
/// does — through the trust record, which is the only authority
/// (gatekeeper G1.1) — without re-running an install.
fn disable_store_source(trust_dir: &Path, base: &Path, source_id: &str) {
    crate::services::hooks::registry::set_session_source_enabled_in(
        trust_dir, base, source_id, false,
    )
    .unwrap();
}

/// #2927: a disabled session plugin contributes no MCP servers even when
/// its approval record still says yes (D4: the state flag wins, the
/// approval and the DB rows stay for a later re-enable).
#[test]
fn disabled_plugin_contributes_no_mcp_servers() {
    let store = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let trust_dir = trust_root.path().join("sess-7");
    let enabled_id = "plugin-aaaa000000000000";
    let disabled_id = "plugin-bbbb000000000000";
    write_store_manifest(
        store.path(),
        enabled_id,
        false,
        r#"[{"name":"enabled","entry":{"command":"bin/enabled"}}]"#,
    );
    write_store_manifest(
        store.path(),
        disabled_id,
        false,
        r#"[{"name":"disabled","entry":{"command":"bin/disabled"}}]"#,
    );
    add_store_entry(store.path(), enabled_id);
    add_store_entry(store.path(), disabled_id);
    record_mcp_trust(&trust_dir, enabled_id, true);
    record_mcp_trust(&trust_dir, disabled_id, true);
    disable_store_source(&trust_dir, store.path(), disabled_id);

    let (servers, roots) = session_plugin_mcp_records(store.path(), Some(&trust_dir));
    let names: Vec<&str> = servers
        .iter()
        .map(|(_, snapshot)| snapshot.name.as_str())
        .collect();
    assert_eq!(names, vec!["enabled"], "disabled plugin servers: {names:?}");
    assert_eq!(servers[0].0.manifest.source_id, enabled_id);
    assert!(
        roots.contains_key(disabled_id),
        "the disabled plugin still shadows the global copy of its source_id"
    );
}

// #2942 S3.2: the session store now lives in the agent-writable
// workspace, so a tampered workspace manifest claiming mcp_approved=true
// must never start a server — only the out-of-workspace trust record
// can approve.
#[tokio::test]
async fn tampered_session_manifest_cannot_start_plugin_mcp_servers() {
    let store = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let trust_dir = trust_root.path().join("sess-9");
    let id = "plugin-cccc000000000000";
    write_store_manifest(
        store.path(),
        id,
        true, // tampered: the agent flipped the workspace flag
        r#"[{"name":"evil","entry":{"command":"never-run"}}]"#,
    );
    add_store_entry(store.path(), id);

    let mut taken = std::collections::HashSet::new();
    // No trust record: tampering alone starts nothing, both with the
    // real (empty) trust dir and with none at all.
    let configs =
        build_session_plugin_server_configs(store.path(), "sess-9", Some(&trust_dir), &mut taken)
            .await;
    assert!(
        configs.is_empty(),
        "workspace tampering must not approve servers"
    );
    let configs =
        build_session_plugin_server_configs(store.path(), "sess-9", None, &mut taken).await;
    assert!(configs.is_empty());

    // Genuine approval through the trust record starts the server.
    record_mcp_trust(&trust_dir, id, true);
    let configs =
        build_session_plugin_server_configs(store.path(), "sess-9", Some(&trust_dir), &mut taken)
            .await;
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].server_name, "evil");
    assert_eq!(configs[0].pseudo_id, "plugin:plugin-cccc000000000000:evil");
}

// #2926 gatekeeper G1.1: a session's option sidecar is read from the
// out-of-workspace trust directory, never from the agent-writable
// workspace plugin store — a sidecar the session's agent writes there
// must never reach `CLAUDE_PLUGIN_OPTION_*` or `${user_config.*}`.
#[tokio::test]
async fn session_plugin_options_come_from_the_trust_dir_not_the_workspace() {
    let store = tempfile::tempdir().unwrap();
    let trust_root = tempfile::tempdir().unwrap();
    let trust_dir = trust_root.path().join("sess-7");
    let id = "plugin-dddd000000000000";
    write_store_manifest(
        store.path(),
        id,
        false,
        r#"[{"name":"srv","entry":{"command":"bin","args":["${user_config.TOKEN}"]}}]"#,
    );
    add_store_entry(store.path(), id);
    record_mcp_trust(&trust_dir, id, true);
    // The genuine values, exactly as a session install leaves them: the
    // sidecar sits in the trust directory.
    std::fs::create_dir_all(&trust_dir).unwrap();
    std::fs::write(
        trust_dir.join(format!("{id}.options.json")),
        r#"{ "values": { "TOKEN": "genuine" }, "sensitiveKeys": [] }"#,
    )
    .unwrap();
    // Tampering attempt: an extra sidecar in the agent-writable store.
    std::fs::write(
        store.path().join(format!("{id}.options.json")),
        r#"{ "values": { "TOKEN": "evil" }, "sensitiveKeys": [] }"#,
    )
    .unwrap();

    let mut taken = std::collections::HashSet::new();
    let configs =
        build_session_plugin_server_configs(store.path(), "sess-7", Some(&trust_dir), &mut taken)
            .await;
    assert_eq!(configs.len(), 1);
    match &configs[0].config.transport {
        crate::mcp::types::TransportConfig::Stdio { args, env, .. } => {
            assert_eq!(
                args,
                &vec!["genuine".to_string()],
                "the workspace sidecar must not expand into the server config"
            );
            assert_eq!(
                env.get("CLAUDE_PLUGIN_OPTION_TOKEN").map(String::as_str),
                Some("genuine"),
                "CLAUDE_PLUGIN_OPTION_* must carry the trust sidecar's value"
            );
            assert!(
                !env.values().any(|value| value == "evil"),
                "a tampered workspace sidecar value must never be exported"
            );
        }
        _ => panic!("expected stdio transport"),
    }
}

async fn marker_count(db: &sea_orm::DatabaseConnection, marker: &str) -> usize {
    use sea_orm::EntityTrait;
    crate::entity::mcp_server::Entity::find()
        .all(db)
        .await
        .unwrap()
        .into_iter()
        .filter(|row| crate::mcp::types::plugin_source_of(&row.config).as_deref() == Some(marker))
        .count()
}

// #2924 (gatekeeper G1.1): re-install / update converges the global store
// exactly on the approved snapshot — create never self-removes, and
// remove deletes every owned row first.
#[tokio::test]
async fn create_and_remove_rows_converge_on_the_approved_snapshot() {
    use sea_orm_migration::MigratorTrait;
    let _guard = crate::state::lock_test_global_state().await;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    crate::state::set_mcp_server_repository(
        crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db.clone()),
    );

    let source_id = "plugin-cc00000000000001";
    let marker = plugin_source_marker(source_id);
    let root = std::path::Path::new("/tmp/plugin-root-under-test");
    let snapshot = |name: &str| PluginMcpServerSnapshot {
        name: name.to_string(),
        entry: serde_json::json!({"command": "true"}),
    };

    // First install: both approved servers become owned rows.
    let warnings =
        create_plugin_mcp_server_rows(source_id, root, &[snapshot("alpha"), snapshot("beta")])
            .await
            .expect("store write ok");
    assert!(warnings.is_empty());
    assert_eq!(marker_count(&db, &marker).await, 2);

    // Update that drops "beta": removal runs first, so only the new
    // snapshot survives (stale rows never keep running).
    remove_global_plugin_mcp_servers(source_id)
        .await
        .expect("removal ok");
    assert_eq!(marker_count(&db, &marker).await, 0);
    let warnings = create_plugin_mcp_server_rows(source_id, root, &[snapshot("alpha")])
        .await
        .expect("store write ok");
    assert!(warnings.is_empty());
    assert_eq!(marker_count(&db, &marker).await, 1);

    // Uninstall removes every owned row of the source.
    remove_global_plugin_mcp_servers(source_id)
        .await
        .expect("removal ok");
    assert_eq!(marker_count(&db, &marker).await, 0);
}

// #2924 (gatekeeper G2.1): the install-path re-sync itself (not just its
// helpers) must remove every owned row BEFORE any early return — an
// unapproved re-install, a dropped server, or an empty snapshot must
// never leave stale rows in the global store.
#[tokio::test]
async fn re_sync_through_the_install_sync_path_converges() {
    use crate::services::hooks::registry::{PluginSourceManifest, PluginSourceRecord};
    use crate::services::skill_service::importing::sync_plugin_mcp_rows_from_record;
    use sea_orm_migration::MigratorTrait;
    let _guard = crate::state::lock_test_global_state().await;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    crate::state::set_mcp_server_repository(
        crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db.clone()),
    );

    let source_id = "plugin-cc00000000000003";
    let marker = plugin_source_marker(source_id);
    let snapshot = |name: &str| PluginMcpServerSnapshot {
        name: name.to_string(),
        entry: serde_json::json!({"command": "true"}),
    };
    let record = |approved: bool, servers: Vec<PluginMcpServerSnapshot>| PluginSourceRecord {
        manifest: PluginSourceManifest {
            source_url: "https://example.com/resync".to_string(),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: Default::default(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            mcp_servers: servers,
            mcp_approved: approved,
            enabled: true,
            installed_commit: None,
        },
        root_dir: std::path::PathBuf::from("/tmp/plugin-root-under-test"),
    };

    // (1) Approved install with two servers → two owned rows.
    sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha"), snapshot("beta")]))
        .await
        .expect("sync ok");
    assert_eq!(marker_count(&db, &marker).await, 2);

    // (2) Re-sync without approval → every stale row is gone, none created.
    sync_plugin_mcp_rows_from_record(&record(false, vec![snapshot("alpha"), snapshot("beta")]))
        .await
        .expect("sync ok");
    assert_eq!(marker_count(&db, &marker).await, 0);

    // Re-approve to seed the rows again, then (3) re-sync with "beta"
    // dropped → exactly the new snapshot survives.
    sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha"), snapshot("beta")]))
        .await
        .expect("sync ok");
    assert_eq!(marker_count(&db, &marker).await, 2);
    sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha")]))
        .await
        .expect("sync ok");
    assert_eq!(marker_count(&db, &marker).await, 1);

    // (4) Re-sync with an empty snapshot → zero rows.
    sync_plugin_mcp_rows_from_record(&record(true, vec![]))
        .await
        .expect("sync ok");
    assert_eq!(marker_count(&db, &marker).await, 0);
}

// #2924 (gatekeeper G1.2): an approved server that cannot be installed
// must come back as a reported warning — never a silent drop.
#[tokio::test]
async fn uninstallable_server_reports_a_warning() {
    use sea_orm_migration::MigratorTrait;
    let _guard = crate::state::lock_test_global_state().await;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    crate::state::set_mcp_server_repository(
        crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db),
    );

    let root = std::path::Path::new("/tmp/plugin-root-under-test");
    let good = PluginMcpServerSnapshot {
        name: "good".to_string(),
        entry: serde_json::json!({"command": "true"}),
    };
    let broken = PluginMcpServerSnapshot {
        name: "broken".to_string(),
        entry: serde_json::json!({"noTransport": true}),
    };
    let warnings = create_plugin_mcp_server_rows("plugin-cc00000000000002", root, &[good, broken])
        .await
        .expect("the store itself stays healthy; only the row fails");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("broken"), "{warnings:?}");
    assert!(
        warnings[0].contains("could not be installed"),
        "{warnings:?}"
    );
}

use super::*;

#[test]
fn matches_target_by_id_url_or_repo_name() {
    let record = PluginSourceRecord {
        manifest: PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "https://github.com/obra/Superpowers/".to_string(),
            source_id: "plugin-0123456789abcdef".to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: Default::default(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
        root_dir: PathBuf::new(),
    };
    assert!(matches_target(&record, "plugin-0123456789abcdef"));
    assert!(matches_target(
        &record,
        "https://github.com/obra/superpowers"
    ));
    assert!(matches_target(&record, "superpowers"));
    assert!(!matches_target(&record, "other"));
}

#[test]
fn enable_target_resolves_session_then_global() {
    let fx = fixture();
    let global = tempfile::tempdir().unwrap();
    let shared = "https://github.com/acme/shared";
    let shared_id = plugin_source_id(shared);
    write_source_manifest(&fx.dirs.plugins, &shared_id, shared, true);
    write_source_manifest(global.path(), &shared_id, shared, true);
    let global_only = "https://github.com/acme/global-only";
    let global_only_id = plugin_source_id(global_only);
    write_source_manifest(global.path(), &global_only_id, global_only, false);

    // Same source in both scopes: the session copy answers.
    assert_eq!(
        resolve_plugin_target(&fx.dirs, global.path(), shared).unwrap(),
        (PluginScope::Session, shared_id.clone())
    );
    // Session plugins are matched by id and repo name too.
    assert_eq!(
        resolve_plugin_target(&fx.dirs, global.path(), "shared").unwrap(),
        (PluginScope::Session, shared_id.clone())
    );
    // Nothing session-scoped matches, so the global copy answers.
    assert_eq!(
        resolve_plugin_target(&fx.dirs, global.path(), "global-only").unwrap(),
        (PluginScope::Global, global_only_id.clone())
    );
    assert_eq!(
        resolve_plugin_target(&fx.dirs, global.path(), global_only).unwrap(),
        (PluginScope::Global, global_only_id)
    );

    // Two session installs matching one name is ambiguous, not a guess.
    write_source_manifest(
        &fx.dirs.plugins,
        "plugin-1111111111111111",
        "https://github.com/other/shared",
        true,
    );
    let err = resolve_plugin_target(&fx.dirs, global.path(), "shared").unwrap_err();
    assert!(err.contains("ambiguous"), "{err}");

    // A target nothing knows is an error that says so.
    let err = resolve_plugin_target(&fx.dirs, global.path(), "missing").unwrap_err();
    assert!(err.contains("No plugin matching"), "{err}");
}

#[test]
fn overview_dedups_global_and_session_with_session_winning() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("session-hook"));
    install_from_dir(&fx, &repo, true).unwrap();
    let session_entry = &overview_for(&fx, Path::new("/nonexistent")).active[0];

    let global = tempfile::tempdir().unwrap();
    // Same source id as the session install => shadowed.
    write_global_source(
        global.path(),
        &session_entry.id,
        &session_entry.source_url,
        "global-hook",
    );
    // A different global plugin stays visible.
    write_global_source(
        global.path(),
        "plugin-0000000000000001",
        "https://github.com/o/other",
        "other",
    );

    let overview = overview_for(&fx, global.path());
    let scopes: Vec<_> = overview
        .active
        .iter()
        .map(|e| (e.id.as_str(), e.scope))
        .collect();
    assert_eq!(overview.active.len(), 2, "no duplicate entry: {scopes:?}");
    assert_eq!(overview.active[0].scope, PluginScope::Session);
    assert_eq!(overview.active[1].id, "plugin-0000000000000001");
    assert_eq!(overview.active[1].scope, PluginScope::Global);
}

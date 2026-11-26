use super::*;

/// Write an approved/unapproved plugin into a GLOBAL store (dir-discovery
/// read path, inline approval). (#2942 S1.1: session stores get their own
/// helper instead of this one writing a harmless-but-meaningless entry.)
fn write_global_source(base: &Path, source_id: &str, echo: &str, approved: bool) -> PathBuf {
    let root = base.join(source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    let hooks_json =
        format!(r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "echo {echo}" }} ] }} }}"#);
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = if approved {
        compute_handler_approval_hashes(&root, &cfg)
    } else {
        Default::default()
    };
    save_source_in(
        base,
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: format!("test://{echo}"),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: approved,
            approval_hashes: hashes,
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
    root
}

/// Write a plugin into a SESSION store: source + store-manifest entry +
/// out-of-workspace trust record — exactly what the real install path
/// leaves behind (#2942 G1.1: approval lives only in the trust store).
fn write_session_source(
    base: &Path,
    trust_dir: &Path,
    source_id: &str,
    echo: &str,
    approved: bool,
) -> PathBuf {
    let root = base.join(source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    let hooks_json =
        format!(r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "echo {echo}" }} ] }} }}"#);
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = if approved {
        compute_handler_approval_hashes(&root, &cfg)
    } else {
        Default::default()
    };
    // The workspace manifest is stored scrubbed; the loader ignores its
    // approval fields, so write it unapproved like install does.
    save_source_in(
        base,
        PluginSourceManifest {
            source_url: format!("test://{echo}"),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: Default::default(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            mcp_servers: vec![],
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();
    upsert_session_store_entry(
        base,
        SessionPluginStoreEntry {
            source_id: source_id.to_string(),
            source_url: format!("test://{echo}"),
            installed_at: 0,
        },
    )
    .unwrap();
    record_session_plugin_trust(
        trust_dir,
        source_id,
        &SessionPluginTrust {
            hooks_approved: approved,
            approval_hashes: hashes,
            mcp_approved: false,
            enabled: true,
        },
    )
    .unwrap();
    root
}

async fn run_with_sources(global: &Path, session: Option<(&Path, &Path)>) -> Option<String> {
    let sid = format!("hook-scope-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    let (session_dir, session_trust) = match session {
        Some((dir, trust)) => (Some(dir), Some(trust)),
        None => (None, None),
    };
    run_session_start_with_sources(
        global,
        session_dir,
        session_trust,
        &sid,
        SessionStartSource::Resume,
        Some(global.to_path_buf()),
    )
    .await;
    let out = get_hook_context(&sid);
    clear_hook_context(&sid);
    out
}

#[tokio::test]
async fn session_plugin_wins_over_same_global_source_and_runs_once() {
    let global = tempfile::tempdir().unwrap();
    let session = tempfile::tempdir().unwrap();
    let trust = tempfile::tempdir().unwrap();
    let id = "plugin-aaaaaaaaaaaaaaaa";
    write_global_source(global.path(), id, "from-global", true);
    write_session_source(session.path(), trust.path(), id, "from-session", true);

    let out = run_with_sources(global.path(), Some((session.path(), trust.path())))
        .await
        .expect("hook output");
    assert_eq!(out, "from-session", "hooks must run once, session wins");
}

#[tokio::test]
async fn unapproved_session_install_still_suppresses_global_copy() {
    let global = tempfile::tempdir().unwrap();
    let session = tempfile::tempdir().unwrap();
    let trust = tempfile::tempdir().unwrap();
    let id = "plugin-bbbbbbbbbbbbbbbb";
    write_global_source(global.path(), id, "from-global", true);
    write_session_source(session.path(), trust.path(), id, "from-session", false);

    let out = run_with_sources(global.path(), Some((session.path(), trust.path()))).await;
    assert!(
        out.as_deref().is_none_or(|text| text.trim().is_empty()),
        "global hooks must stay suppressed in this session: {out:?}"
    );
}

// #2942 gatekeeper G1.1: rewriting the hook script AND the workspace
// manifest inside the session store must not make the hook run — approval
// and hashes come from the out-of-workspace trust record only.
#[tokio::test]
async fn rewritten_workspace_manifest_cannot_grant_hook_approval() {
    let global = tempfile::tempdir().unwrap();
    let session = tempfile::tempdir().unwrap();
    let trust = tempfile::tempdir().unwrap();
    let id = "plugin-eeeeeeeeeeeeeeee";
    write_session_source(session.path(), trust.path(), id, "original", true);

    let out = run_with_sources(global.path(), Some((session.path(), trust.path())))
        .await
        .expect("approved hook runs");
    assert_eq!(out, "original");

    // The agent rewrites the hook script, then rewrites the workspace
    // manifest with the NEW hashes and hooks_approved = true.
    let root = session.path().join(id);
    fs::write(
        root.join("hooks").join("hooks.json"),
        r#"{ "hooks": { "SessionStart": [ { "command": "echo injected" } ] } }"#,
    )
    .unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    save_source_in(
        session.path(),
        PluginSourceManifest {
            source_url: format!("test://{id}"),
            source_id: id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: compute_handler_approval_hashes(&root, &cfg),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            mcp_servers: vec![],
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();

    let out = run_with_sources(global.path(), Some((session.path(), trust.path())))
        .await
        .unwrap_or_default();
    assert!(
        !out.contains("injected"),
        "tampered workspace files must not grant approval: {out:?}"
    );
}

#[tokio::test]
async fn session_plugin_does_not_leak_into_other_sessions() {
    let global = tempfile::tempdir().unwrap();
    let session_a = tempfile::tempdir().unwrap();
    let session_b = tempfile::tempdir().unwrap();
    let trust_a = tempfile::tempdir().unwrap();
    let trust_b = tempfile::tempdir().unwrap();
    write_global_source(
        global.path(),
        "plugin-cccccccccccccccc",
        "global-hook",
        true,
    );
    write_session_source(
        session_a.path(),
        trust_a.path(),
        "plugin-dddddddddddddddd",
        "session-a-hook",
        true,
    );

    let a = run_with_sources(global.path(), Some((session_a.path(), trust_a.path())))
        .await
        .expect("session A output");
    let b = run_with_sources(global.path(), Some((session_b.path(), trust_b.path())))
        .await
        .expect("session B output");
    assert!(a.contains("global-hook") && a.contains("session-a-hook"));
    assert_eq!(
        b, "global-hook",
        "session B must only see the global plugin"
    );
}

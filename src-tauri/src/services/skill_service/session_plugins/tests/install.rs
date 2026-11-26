use super::*;

#[test]
fn install_records_scope_url_time_hooks_and_skills() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
    install_from_dir(&fx, &repo, true).unwrap();

    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert_eq!(overview.active.len(), 1);
    let entry = &overview.active[0];
    assert_eq!(entry.scope, PluginScope::Session);
    assert_eq!(entry.source_url, repo.to_string_lossy());
    assert!(entry.installed_at > 0);
    assert_eq!(entry.skill_names, vec!["alpha-skill".to_string()]);
    assert_eq!(entry.hook_events, vec!["SessionStart".to_string()]);
    assert!(entry.hooks_approved);
    assert_eq!(overview.history.len(), 1);
    assert_eq!(overview.history[0].action, SessionPluginAction::Install);
    // Skills land in the session workspace skills dir, not a global dir.
    assert!(fx.dirs.skills.is_dir());
}

#[test]
fn hook_less_skill_bundle_is_still_listable_and_removable() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "bundle", None);
    install_from_dir(&fx, &repo, false).unwrap();

    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert_eq!(overview.active.len(), 1);
    assert!(overview.active[0].hook_events.is_empty());
    assert!(!overview.active[0].hooks_approved);

    uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();
    let overview = overview_for(&fx, global.path());
    assert!(overview.active.is_empty());
}

#[test]
fn sessions_are_isolated_from_each_other() {
    let fx_a = fixture();
    let fx_b = fixture();
    let repo = write_plugin_repo(fx_a.root.path(), "alpha", Some("hi"));
    install_from_dir(&fx_a, &repo, true).unwrap();

    let global = tempfile::tempdir().unwrap();
    let a = overview_for(&fx_a, global.path());
    let b = overview_for(&fx_b, global.path());
    assert_eq!(a.active.len(), 1);
    assert!(b.active.is_empty() && b.history.is_empty());
    assert!(!fx_b.dirs.skills.exists());
}

#[test]
fn install_writes_store_manifest_entry() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", None);
    install_from_dir(&fx, &repo, false).unwrap();

    let entries = read_store_manifest(&fx.dirs.plugins);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source_url, repo.to_string_lossy());
    assert!(entries[0].installed_at > 0);
    assert!(fx.dirs.plugins.join(&entries[0].source_id).exists());
}

#[test]
fn manifest_entry_with_missing_dir_is_skipped_without_failing() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", None);
    install_from_dir(&fx, &repo, false).unwrap();
    let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

    // Simulate the user deleting the plugin directory: the entry is
    // skipped with a log line, the overview still loads (D1 policy).
    fs::remove_dir_all(fx.dirs.plugins.join(&source_id)).unwrap();
    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert!(overview.active.is_empty());
    assert_eq!(overview.history.len(), 1, "history still readable");
}

#[test]
fn directory_without_manifest_entry_is_not_loaded() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", None);
    install_from_dir(&fx, &repo, false).unwrap();
    let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

    // Simulate removing only the manifest entry: the plugin must not load
    // even though its files are still on disk (#2942 load-from-manifest).
    remove_session_store_entry(&fx.dirs.plugins, &source_id).unwrap();
    assert!(fx.dirs.plugins.join(&source_id).is_dir());
    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert!(overview.active.is_empty());
}

#[tokio::test]
async fn session_install_survives_resume_and_runs_hooks() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("restored-bootstrap"));
    install_from_dir(&fx, &repo, true).unwrap();

    // "Resume": a later SessionStart(resume) reads only what is on disk.
    let global = tempfile::tempdir().unwrap();
    let sid = format!("resume-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_with_sources(
        global.path(),
        Some(&fx.dirs.plugins),
        Some(&fx.dirs.trust),
        &sid,
        SessionStartSource::Resume,
        Some(fx.root.path().to_path_buf()),
    )
    .await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("restored-bootstrap")
    );
    clear_hook_context(&sid);
}

#[tokio::test]
async fn unapproved_session_install_runs_no_hooks() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("must-not-run"));
    install_from_dir(&fx, &repo, false).unwrap();

    let global = tempfile::tempdir().unwrap();
    let sid = format!("unapproved-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_with_sources(
        global.path(),
        Some(&fx.dirs.plugins),
        Some(&fx.dirs.trust),
        &sid,
        SessionStartSource::Resume,
        Some(fx.root.path().to_path_buf()),
    )
    .await;
    assert!(get_hook_context(&sid).is_none_or(|t| t.trim().is_empty()));
    clear_hook_context(&sid);
}

// --- #2942 gatekeeper G1.1 / G1.2 -----------------------------------------

#[test]
fn install_moves_approval_out_of_the_workspace_manifest() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
    install_from_dir(&fx, &repo, true).unwrap();
    let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

    // The workspace manifest on disk grants nothing: approval fields are
    // scrubbed at install, so a rewritten workspace file alone can never
    // re-approve hooks (#2942 gatekeeper G1.1).
    let on_disk = get_source_in(&fx.dirs.plugins, &source_id).unwrap();
    assert!(!on_disk.manifest.hooks_approved);
    assert!(on_disk.manifest.approval_hashes.is_empty());

    // Approval lives only in the out-of-workspace trust record, and its
    // hashes still match the installed handlers.
    let trust = read_session_plugin_trust(&fx.dirs.trust, &source_id);
    assert!(trust.hooks_approved);
    let root = fx.dirs.plugins.join(&source_id);
    let cfg = load_hooks_from_root(&root).unwrap();
    assert_eq!(
        trust.approval_hashes,
        compute_handler_approval_hashes(&root, &cfg)
    );

    // The single reader overlays the trust back onto the record.
    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert!(overview.active[0].hooks_approved);
}

#[test]
fn blocked_store_manifest_fails_install_and_cleans_up() {
    let fx = fixture();
    // A directory sitting where store-manifest.json must be written makes
    // every manifest write fail. The install must report failure and roll
    // back the just-written source instead of claiming success (#2942
    // gatekeeper G1.2).
    fs::create_dir_all(fx.dirs.plugins.join(SESSION_STORE_MANIFEST_FILE)).unwrap();
    let repo = write_plugin_repo(fx.root.path(), "alpha", None);
    let err = install_from_dir(&fx, &repo, false).unwrap_err();
    assert!(err.contains("store manifest"), "{err}");
    assert!(
        list_sources_in(&fx.dirs.plugins).unwrap().is_empty(),
        "a rolled-back install must not leave the plugin source behind"
    );
    if fx.dirs.trust.exists() {
        assert_eq!(fs::read_dir(&fx.dirs.trust).unwrap().count(), 0);
    }
}

// --- #2926 gatekeeper G1.2 / G1.3 / G1.4 AC5 ------------------------------

use super::*;

#[test]
fn uninstall_by_url_removes_source_skills_and_logs_history() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", Some("hi"));
    install_from_dir(&fx, &repo, true).unwrap();
    let skills_before = fs::read_dir(&fx.dirs.skills).unwrap().count();
    assert_eq!(skills_before, 1);

    let removed = uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();
    assert_eq!(removed.skill_names, vec!["alpha-skill".to_string()]);
    assert_eq!(fs::read_dir(&fx.dirs.skills).unwrap().count(), 0);

    let global = tempfile::tempdir().unwrap();
    let overview = overview_for(&fx, global.path());
    assert!(overview.active.is_empty());
    let actions: Vec<_> = overview.history.iter().map(|h| h.action).collect();
    assert_eq!(
        actions,
        vec![SessionPluginAction::Install, SessionPluginAction::Uninstall]
    );
}

#[test]
fn uninstall_unknown_target_errors() {
    let fx = fixture();
    let err = uninstall_with_dirs(&fx.dirs, "nope").unwrap_err();
    assert!(err.contains("No plugin matching"));
    assert!(uninstall_with_dirs(&fx.dirs, "  ").is_err());
}

#[test]
fn uninstall_removes_manifest_entry_and_files() {
    let fx = fixture();
    let repo = write_plugin_repo(fx.root.path(), "alpha", None);
    install_from_dir(&fx, &repo, false).unwrap();
    let source_id = read_store_manifest(&fx.dirs.plugins)[0].source_id.clone();

    uninstall_with_dirs(&fx.dirs, &repo.to_string_lossy()).unwrap();

    assert!(read_store_manifest(&fx.dirs.plugins).is_empty());
    assert!(!fx.dirs.plugins.join(&source_id).exists());
}

/// AC5 + G1.2: the cleanup body a session uninstall runs (with a test-
/// injected data dir, since `get_session_plugin_data_dir` needs a real
/// session manager) removes the plugin's `${CLAUDE_PLUGIN_DATA}` tree,
/// its trust-dir sidecar, and exactly its own session-scoped secrets.
#[tokio::test]
async fn session_uninstall_removes_data_dir_sidecar_and_scoped_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let session = "uninstall-cleanup-s1";
    let source_id = "plugin-uninstall-cleanup";
    let trust_dir = temp.path().join("session_plugin_trust").join(session);
    let data_dir = temp
        .path()
        .join("plugin_data/sessions")
        .join(session)
        .join(source_id);
    fs::create_dir_all(data_dir.join("keep")).unwrap();
    fs::write(data_dir.join("keep/state.txt"), "state").unwrap();

    let manifest = sensitive_manifest();
    let values = BTreeMap::from([("token".to_string(), "s3cr3t".to_string())]);
    let saved = crate::services::skill_service::plugin_options::persist_options(
        source_id,
        Some(session),
        &manifest,
        &values,
    )
    .await
    .unwrap();
    crate::services::skill_service::plugin_options::write_saved_options(
        &trust_dir, source_id, &saved,
    )
    .unwrap();
    crate::services::skill_service::plugin_options::clear_cache_for_test();

    cleanup_removed_session_plugin(&trust_dir, Some(&data_dir), session, source_id).await;

    assert!(
        !data_dir.exists(),
        "AC5: a session uninstall must remove the plugin's `${{CLAUDE_PLUGIN_DATA}}` directory"
    );
    assert!(
        !crate::services::skill_service::plugin_options::options_path(&trust_dir, source_id)
            .exists(),
        "the trust-dir option sidecar must go with the plugin"
    );
    let (after, _) = crate::services::skill_service::plugin_options::load_option_values(
        &trust_dir,
        source_id,
        Some(session),
    )
    .await;
    assert!(
        !after.contains_key("token"),
        "G1.2: a session uninstall must delete its session-scoped secret"
    );
}

/// G1.3: session deletion leaves no plugin runtime state behind — not
/// the env-file directory, not the session-scope plugin data tree, not
/// the in-memory env variables, and not the session-scoped secrets.
#[tokio::test]
async fn purge_session_plugin_state_leaves_nothing_behind() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    let session = "purge-s1";
    let source_id = "plugin-purge-test";
    let trust_dir = base
        .join(crate::services::hooks::registry::SESSION_PLUGIN_TRUST_DIR_NAME)
        .join(session);
    let env_dir = base
        .join(crate::services::hooks::registry::PLUGIN_ENV_DIR_NAME)
        .join(session);
    let data_dir = base
        .join(crate::services::hooks::registry::PLUGIN_DATA_DIR_NAME)
        .join("sessions")
        .join(session)
        .join(source_id);
    for dir in [&trust_dir, &env_dir, &data_dir] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(env_dir.join("session-start.env"), "A=1").unwrap();
    fs::write(data_dir.join("blob"), "x").unwrap();
    crate::services::hooks::plugin_env::set_session_plugin_env(
        session,
        BTreeMap::from([("A".to_string(), "1".to_string())]),
    );

    let manifest = sensitive_manifest();
    let values = BTreeMap::from([("token".to_string(), "purge-me".to_string())]);
    let saved = crate::services::skill_service::plugin_options::persist_options(
        source_id,
        Some(session),
        &manifest,
        &values,
    )
    .await
    .unwrap();
    crate::services::skill_service::plugin_options::write_saved_options(
        &trust_dir, source_id, &saved,
    )
    .unwrap();
    // Prime the option cache so the purge must clear it (S1.3).
    let (before, _) = crate::services::skill_service::plugin_options::load_option_values(
        &trust_dir,
        source_id,
        Some(session),
    )
    .await;
    assert_eq!(before.get("token").map(String::as_str), Some("purge-me"));

    purge_session_plugin_state(base, session).await.unwrap();

    assert!(!env_dir.exists(), "the CLAUDE_ENV_FILE dir must not linger");
    assert!(
        !base
            .join(crate::services::hooks::registry::PLUGIN_DATA_DIR_NAME)
            .join("sessions")
            .join(session)
            .exists(),
        "the session-scope plugin data tree must not linger"
    );
    assert!(
        crate::services::hooks::plugin_env::session_plugin_env(session).is_empty(),
        "the in-memory env variables must not linger"
    );
    crate::services::skill_service::plugin_options::clear_cache_for_test();
    let (after, _) = crate::services::skill_service::plugin_options::load_option_values(
        &trust_dir,
        source_id,
        Some(session),
    )
    .await;
    assert!(
        !after.contains_key("token"),
        "session deletion must remove the session-scoped secrets"
    );
}

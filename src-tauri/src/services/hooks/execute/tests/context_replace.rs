use super::*;

#[tokio::test]
async fn failed_compact_rerun_keeps_previous_context() {
    // #2812 G3.2: a compaction re-run that fails (exit 2) must not erase
    // the bootstrap already in effect for the session.
    let hooks_json =
        r#"{ "hooks": { "SessionStart": [ { "command": "echo first bootstrap" } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-keep-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(
        temp.path(),
        &sid,
        SessionStartSource::Startup,
        Some(root.clone()),
    )
    .await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("first bootstrap"),
        "first run stores its output"
    );

    #[cfg(target_os = "windows")]
    let failing = r#"{ "hooks": { "SessionStart": [ { "command": "cmd /c exit 2" } ] } }"#;
    #[cfg(not(target_os = "windows"))]
    let failing = r#"{ "hooks": { "SessionStart": [ { "command": "exit 2" } ] } }"#;
    reapprove_source(temp.path(), &root, failing);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Compact, Some(root)).await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("first bootstrap"),
        "failed re-run must keep the previous context"
    );
    clear_hook_context(&sid);
}

#[tokio::test]
async fn successful_compact_rerun_replaces_previous_context() {
    // #2812 G3.2 counterpart: a successful re-run replaces (not appends
    // to) the previous text, so no duplicate bootstrap accumulates.
    let hooks_json =
        r#"{ "hooks": { "SessionStart": [ { "command": "echo first bootstrap" } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-replace-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(
        temp.path(),
        &sid,
        SessionStartSource::Startup,
        Some(root.clone()),
    )
    .await;

    let updated = r#"{ "hooks": { "SessionStart": [ { "command": "echo second bootstrap" } ] } }"#;
    reapprove_source(temp.path(), &root, updated);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Compact, Some(root)).await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("second bootstrap"),
        "successful re-run must replace the previous context"
    );
    clear_hook_context(&sid);
}

#[tokio::test]
async fn slow_hook_wait_lands_context_before_prompt() {
    // #2812 G3.1: while a hook run is in flight, await_hook_run (the
    // prompt-build gate) waits for it and the bootstrap is then visible.
    #[cfg(target_os = "windows")]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "ping -n 2 127.0.0.1 >nul && echo slow hook body" } ] } }"#;
    #[cfg(not(target_os = "windows"))]
    let hooks_json =
        r#"{ "hooks": { "SessionStart": [ { "command": "sleep 0.4 && echo slow hook body" } ] } }"#;

    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-slow-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    begin_hook_run(&sid);
    let dir = temp.path().to_path_buf();
    let run_sid = sid.clone();
    let run = tokio::spawn(async move {
        run_session_start_in(&dir, &run_sid, SessionStartSource::Startup, Some(root)).await;
        end_hook_run(&run_sid);
    });

    await_hook_run(&sid).await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("slow hook body"),
        "the first prompt waits for the in-flight hook"
    );
    run.await.unwrap();
    clear_hook_context(&sid);
}

#[tokio::test]
async fn mutated_hooks_json_after_approval_is_skipped() {
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "echo approved" } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let tampered = r#"{ "hooks": { "SessionStart": [ { "command": "echo tampered" } ] } }"#;
    fs::write(root.join("hooks").join("hooks.json"), tampered).unwrap();

    let sid = format!("hook-tamper-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

#[tokio::test]
async fn mutated_script_after_approval_is_skipped() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_id = "plugin-fedcba9876543210".to_string();
    let root = temp.path().join(&source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();

    #[cfg(target_os = "windows")]
    let (script_rel, script_body, hooks_json) = (
        "hooks/run.cmd",
        "@echo off\r\necho approved-script\r\n",
        r#"{ "hooks": { "SessionStart": [ { "command": "hooks\\run.cmd" } ] } }"#,
    );
    #[cfg(not(target_os = "windows"))]
    let (script_rel, script_body, hooks_json) = (
        "hooks/run.sh",
        "#!/bin/sh\necho approved-script\n",
        r#"{ "hooks": { "SessionStart": [ { "command": "sh hooks/run.sh" } ] } }"#,
    );

    let script_path = root.join(script_rel);
    fs::write(&script_path, script_body).unwrap();
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();

    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = compute_handler_approval_hashes(&root, &cfg);
    save_source_in(
        temp.path(),
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "test://local".to_string(),
            source_id,
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
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

    // Mutate the referenced script after approval.
    fs::write(&script_path, "echo tampered-script\n").unwrap();

    let sid = format!("hook-script-tamper-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

/// Gatekeeper G2.2: a plugin whose hooks are declared only through
/// `.claude-plugin/plugin.json` must still dispatch. The old runtime
/// loader discovered only `hooks/hooks.json`, so handlers approved at
/// install from a manifest-declared file never ran.
#[tokio::test]
async fn manifest_only_hook_files_are_dispatched() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_id = "plugin-2233445566778899".to_string();
    let root = temp.path().join(&source_id);
    fs::create_dir_all(root.join("custom-hooks")).unwrap();
    fs::write(
        root.join("custom-hooks").join("extra.json"),
        r#"{ "hooks": { "SessionStart": [ { "command": "echo manifest only body" } ] } }"#,
    )
    .unwrap();
    fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    fs::write(
        root.join(".claude-plugin").join("plugin.json"),
        r#"{ "name": "p", "hooks": "./custom-hooks/extra.json" }"#,
    )
    .unwrap();

    let files = vec![root.join("custom-hooks").join("extra.json")];
    let cfg = crate::services::hooks::schema::load_hooks_from_config_files(&files).unwrap();
    let hashes = compute_handler_approval_hashes(&root, &cfg);
    save_source_in(
        temp.path(),
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "test://manifest-only".to_string(),
            source_id,
            skill_names: vec![],
            hooks: Some(crate::services::hooks::registry::HooksConfigSnapshot {
                version: cfg.version,
                validated: true,
            }),
            hooks_approved: true,
            approval_hashes: hashes,
            created_at: 0,
            plugin: None,
            hook_files: vec!["custom-hooks/extra.json".to_string()],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();

    let sid = format!("hook-manifest-only-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert_eq!(
        get_hook_context(&sid).as_deref(),
        Some("manifest only body"),
        "dispatch must load the persisted hook_files, not just hooks/hooks.json"
    );
    clear_hook_context(&sid);
}

#[tokio::test]
async fn pre_tool_use_deny_blocks_matched_tool_and_skips_unmatched() {
    let json_out = r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"blocked by test policy"}}"#;
    let ps = format!("Write-Output '{json_out}'");
    let sh = format!("printf '%s' '{json_out}'");
    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{
                    "type": "command",
                    "command": sh,
                    "powershell": ps,
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("pre-tool-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let matched = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(matched.decision, Some(HookDecision::Block));
    assert_eq!(matched.reason.as_deref(), Some("blocked by test policy"));

    let unmatched = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__readFile",
        &params,
        &sid,
    )
    .await;
    assert_eq!(unmatched.decision, None);
    clear_hook_context(&sid);
}

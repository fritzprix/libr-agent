use super::*;
use crate::services::hooks::execute::matcher::if_condition_matches;

/// #2926: a hook process gets `CLAUDE_PLUGIN_DATA`, `CLAUDE_PROJECT_DIR`,
/// `CLAUDE_ENV_FILE`, and `CLAUDE_PLUGIN_OPTION_<KEY>`; what the hook
/// writes to the env file reaches the session's later shell calls.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn plugin_variables_reach_the_hook_process() {
    let dump_dir = tempfile::tempdir().expect("tempdir");
    let dump_path = dump_dir.path().join("env-dump.txt");
    let hooks_json = format!(
        r#"{{ "hooks": {{ "SessionStart": [ {{ "command": "printf '%s|%s|%s|%s\n' \"$CLAUDE_PLUGIN_DATA\" \"$CLAUDE_PROJECT_DIR\" \"$CLAUDE_ENV_FILE\" \"$CLAUDE_PLUGIN_OPTION_GREETING\" > {dump}; printf 'PLUGIN_TOKEN=from-hook\n' >> \"$CLAUDE_ENV_FILE\"" }} ] }} }}"#,
        dump = dump_path.to_string_lossy()
    );
    let (temp, root) = setup_approved_plugin(&hooks_json);
    // The plugin's non-sensitive `userConfig` value, as an install leaves it.
    let source_id = root.file_name().unwrap().to_string_lossy().to_string();
    std::fs::write(
        temp.path().join(format!("{source_id}.options.json")),
        r#"{ "values": { "greeting": "hello" }, "sensitiveKeys": [] }"#,
    )
    .unwrap();
    // The session's workspace: the directory the session works in, which
    // is a different place from the plugin's install root.
    let workspace = temp.path().join("session-workspace");
    fs::create_dir_all(&workspace).unwrap();

    let sid = format!("hook-env-{}", uuid::Uuid::new_v4());
    crate::services::hooks::plugin_env::clear_session_plugin_env(&sid);
    run_session_start_in(
        temp.path(),
        &sid,
        SessionStartSource::Startup,
        Some(workspace.clone()),
    )
    .await;

    let dump = std::fs::read_to_string(&dump_path).expect("hook wrote its environment");
    let parts = dump.trim().split('|').collect::<Vec<_>>();
    assert_eq!(parts.len(), 4, "unexpected dump: {dump}");

    // `${CLAUDE_PLUGIN_DATA}` exists and is writable, and the data written
    // there survives (it is a real directory, not a temp file).
    let data_dir = std::path::PathBuf::from(parts[0]);
    assert!(!parts[0].is_empty(), "CLAUDE_PLUGIN_DATA was not set");
    std::fs::write(data_dir.join("kept.txt"), "kept").unwrap();
    assert_eq!(
        std::fs::read_to_string(data_dir.join("kept.txt")).unwrap(),
        "kept"
    );

    // AC2 (#2926 gatekeeper G1.4): `CLAUDE_PROJECT_DIR` is the session's
    // workspace — the directory the session works in — and must be
    // distinct from the plugin's install root.
    assert_eq!(std::path::PathBuf::from(parts[1]), workspace);
    assert_ne!(
        std::path::PathBuf::from(parts[1]),
        root,
        "CLAUDE_PROJECT_DIR must be the session workspace, not the plugin root"
    );

    // `CLAUDE_ENV_FILE` is a real file the hook appended to.
    assert!(!parts[2].is_empty(), "CLAUDE_ENV_FILE was not set");
    assert!(std::path::Path::new(parts[2]).is_file());

    // The plugin option is exported with the documented name.
    assert_eq!(parts[3], "hello");

    // The variable the hook wrote to the env file is available to the
    // session afterwards, and a shell spawn merges it into its env.
    let vars = crate::services::hooks::plugin_env::session_plugin_env(&sid);
    assert_eq!(
        vars.get("PLUGIN_TOKEN").map(String::as_str),
        Some("from-hook")
    );
    let shell_manager =
        crate::mcp::builtin::workspace::persistent_shell::PersistentShellManager::new();
    let secrets = shell_manager.session_secrets(&sid).await;
    assert_eq!(
        secrets.get("PLUGIN_TOKEN").map(String::as_str),
        Some("from-hook"),
        "a later shell call of the session sees the env-file variable"
    );

    crate::services::hooks::plugin_env::clear_session_plugin_env(&sid);
}

#[test]
fn matcher_or_patterns() {
    assert!(matcher_matches(Some("startup|clear"), "startup"));
    assert!(!matcher_matches(Some("startup|clear"), "resume"));
    assert!(matcher_matches(None, "anything"));
    assert!(matcher_matches(Some(""), "anything"));
    assert!(matcher_matches(Some("*"), "anything"));
}

#[test]
fn matcher_exact_lists_and_regex_contract() {
    // Exact-match character set: | and , separated exact strings
    assert!(matcher_matches(Some("Edit|Write"), "Edit"));
    assert!(matcher_matches(Some("Edit|Write"), "Write"));
    assert!(!matcher_matches(Some("Edit|Write"), "EditFile"));
    assert!(matcher_matches(Some("Edit, Write"), "Edit"));
    assert!(matcher_matches(Some("Edit, Write"), "Write"));
    assert!(!matcher_matches(Some("Edit, Write"), "Read"));

    // Regex characters fall back to unanchored regex
    assert!(matcher_matches(Some("^Notebook"), "NotebookEdit"));
    assert!(!matcher_matches(Some("^Notebook"), "MyNotebook"));
}

#[test]
fn if_condition_permission_rule_matching() {
    let git_status = serde_json::json!({ "command": "git status" });
    let npm_test = serde_json::json!({ "command": "npm test" });

    // Dispatch passes real LibrAgent tool names (#3003 G1.4), so the
    // Claude name `Bash` must map onto `workspace__runShell`.
    assert!(if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&git_status),
    ));
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&npm_test),
    ));
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__writeFile"),
        Some(&git_status),
    ));
    // Rules may also name the LibrAgent tool directly.
    assert!(if_condition_matches(
        Some("workspace__runShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&git_status),
    ));
    // A bare tool rule matches through the alias too.
    assert!(if_condition_matches(
        Some("Bash"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        None,
    ));
    // Omitted `if` matches everything
    assert!(if_condition_matches(
        None,
        HookEvent::PreToolUse,
        Some("workspace__writeFile"),
        Some(&npm_test),
    ));
}

#[test]
fn if_condition_bash_subcommands_expansions_and_failures() {
    let run = |rule: &str, command: &str| {
        if_condition_matches(
            Some(rule),
            HookEvent::PreToolUse,
            Some("workspace__runShell"),
            Some(&serde_json::json!({ "command": command })),
        )
    };
    // Documented table: each subcommand is checked, leading assignments
    // are stripped.
    assert!(run("Bash(git *)", "npm test && git push"));
    assert!(run("Bash(git *)", "FOO=bar git push"));
    // `$()` and backtick contents are checked.
    assert!(run("Bash(rm *)", "echo $(rm -rf /tmp/build)"));
    assert!(run("Bash(rm *)", "echo `rm -rf /tmp/build`"));
    assert!(!run("Bash(rm *)", "echo $(date)"));
    // A pattern naming more than the command runs anyway on expansions.
    assert!(run("Bash(git push *)", "echo $(date)"));
    // Multi-line commands: `.*` must cross newlines.
    assert!(run("Bash(git *)", "echo one\ngit push"));
    // Unparseable commands run the guard rather than skip it.
    assert!(run("Bash(git *)", "echo \"unclosed"));
    assert!(run("Bash(git *)", "echo $(unbalanced"));
    // Genuine misses still miss.
    assert!(!run("Bash(git *)", "npm test && cargo build"));
}

/// Every construct the segment splitter does not fully model must run the
/// guard hook instead of silently skipping it (#3003 G2.1) — a skipped
/// PreToolUse guard is a bypass.
#[test]
fn if_condition_bash_unmodeled_constructs_run_the_guard() {
    let run = |rule: &str, command: &str| {
        if_condition_matches(
            Some(rule),
            HookEvent::PreToolUse,
            Some("workspace__runShell"),
            Some(&serde_json::json!({ "command": command })),
        )
    };
    // A lone `&` backgrounds the first segment; `rm` still runs.
    assert!(run("Bash(rm *)", "echo hi & rm -rf x"));
    // Grouping and control flow keep `rm` reachable at runtime.
    assert!(run("Bash(rm *)", "(rm -rf x)"));
    assert!(run("Bash(rm *)", "{ rm -rf x; }"));
    assert!(run("Bash(rm *)", "if true; then rm -rf x; fi"));
    // Backslash escapes must not hide a segment from the guard.
    // (Rust-escaped: the shell command contains literal backslashes.)
    assert!(run("Bash(rm *)", "echo \\\"; rm -rf x; echo \\\""));
    // A `'` inside double quotes is a literal, not a quote span.
    assert!(run(
        "Bash(rm *)",
        "echo \"it's\" && rm -rf x && echo \"it's\""
    ));
    // Round 3 (#3003 G2.1): the command word itself must be a plain
    // unquoted word, or the whole command is unjudgeable and the guard
    // runs. Quoted, reserved-word, negated, or redirection-led command
    // words all fall here.
    assert!(run("Bash(rm *)", "\"rm\" -rf x"));
    assert!(run("Bash(rm *)", "'rm' -rf x"));
    assert!(run("Bash(rm *)", "r''m -rf x"));
    assert!(run("Bash(rm *)", "! rm -rf x"));
    assert!(run("Bash(rm *)", "time rm -rf x"));
    assert!(run("Bash(rm *)", ">log rm -rf x"));
}

/// `Bash(...)`/`PowerShell(...)` rules must guard the Windows shell tools
/// too — the predicate is the shell policy's own list, single source
/// (#3003 G2.2).
#[test]
fn if_condition_bash_rules_guard_powershell_tools() {
    let git_status = serde_json::json!({ "command": "git status" });
    assert!(if_condition_matches(
        Some("PowerShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runPowerShell"),
        Some(&git_status),
    ));
    assert!(if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runInPersistentPowerShell"),
        Some(&git_status),
    ));
    assert!(!if_condition_matches(
        Some("PowerShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runPowerShell"),
        Some(&serde_json::json!({ "command": "npm test" })),
    ));
}

#[test]
fn if_condition_on_non_tool_event_does_not_run() {
    // Hooks reference: on events other than tool events, a handler with
    // `if` set does not run (#3003 G1.3).
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::SessionStart,
        None,
        None,
    ));
    // Without `if`, non-tool events are unaffected.
    assert!(if_condition_matches(
        None,
        HookEvent::SessionStart,
        None,
        None,
    ));
}

#[test]
fn if_condition_matches_file_tools_on_path_argument() {
    assert!(if_condition_matches(
        Some("Edit(*.rs)"),
        HookEvent::PreToolUse,
        Some("workspace__strReplace"),
        Some(&serde_json::json!({
            "path": "src/main.rs",
            "old_string": "a",
            "new_string": "b"
        })),
    ));
    assert!(!if_condition_matches(
        Some("Edit(*.rs)"),
        HookEvent::PreToolUse,
        Some("workspace__strReplace"),
        Some(&serde_json::json!({ "path": "README.md" })),
    ));
}

/// Same event, matcher, and command with two different `if` values — the
/// documented way to express two conditions. If the approval key ignored
/// `if`, the second handler's hash would overwrite the first's and one
/// handler would be skipped as "unapproved or modified" (#3003 G1.1).
#[tokio::test]
async fn two_handlers_differing_only_by_if_each_get_dispatched() {
    let json_out = r#"{"additionalContext":"guarded"}"#;
    let ps = format!("Write-Output '{json_out}'");
    let sh = format!("printf '%s' '{json_out}'");
    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [
                    { "type": "command", "command": sh, "powershell": ps, "if": "Bash(git *)" },
                    { "type": "command", "command": sh, "powershell": ps, "if": "Bash(npm *)" }
                ]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("if-approval-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    params.tool_name = Some("workspace__runShell".to_string());

    params.tool_input = Some(serde_json::json!({ "command": "git status" }));
    let git = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(
        git.additional_context,
        vec!["guarded".to_string()],
        "the `git *` handler must be approved and run"
    );

    params.tool_input = Some(serde_json::json!({ "command": "npm test" }));
    let npm = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(
        npm.additional_context,
        vec!["guarded".to_string()],
        "the `npm *` handler must be approved and run"
    );
}

#[test]
fn all_supported_events_use_execute_dispatch_mode() {
    for event in [
        HookEvent::SessionStart,
        HookEvent::SessionEnd,
        HookEvent::UserPromptSubmit,
        HookEvent::PreToolUse,
        HookEvent::PostToolUse,
        HookEvent::Stop,
        HookEvent::SubagentStop,
        HookEvent::PreCompact,
        HookEvent::Notification,
    ] {
        assert_eq!(event.dispatch_mode(), HookDispatchMode::Execute);
    }
}

#[tokio::test]
async fn session_start_plain_text_context_stored() {
    let hooks_json =
        r#"{ "hooks": { "SessionStart": [ { "command": "echo plain hook body" } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-test-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert_eq!(get_hook_context(&sid).as_deref(), Some("plain hook body"));
    clear_hook_context(&sid);
}

#[tokio::test]
async fn session_start_json_context_stored() {
    #[cfg(target_os = "windows")]
    let (temp, root) = {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_id = "plugin-1122334455667788".to_string();
        let root = temp.path().join(&source_id);
        fs::create_dir_all(root.join("hooks")).unwrap();
        fs::write(
            root.join("hooks").join("emit_json.cmd"),
            "@echo off\r\necho {\"additionalContext\":\"json hook body\"}\r\n",
        )
        .unwrap();
        let hooks_json =
            r#"{ "hooks": { "SessionStart": [ { "command": "hooks\\emit_json.cmd" } ] } }"#;
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
        (temp, root)
    };
    #[cfg(not(target_os = "windows"))]
    let (temp, root) = {
        let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "printf '%s\\n' '{\"additionalContext\":\"json hook body\"}'" } ] } }"#;
        setup_approved_plugin(hooks_json)
    };

    let sid = format!("hook-json-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert_eq!(get_hook_context(&sid).as_deref(), Some("json hook body"));
    clear_hook_context(&sid);
}

#[tokio::test]
async fn session_start_non_matching_matcher_skipped() {
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "matcher": "resume", "hooks": [ { "type": "command", "command": "echo should not match" } ] } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-matcher-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

#[tokio::test]
async fn session_start_timeout_is_killed_and_skipped() {
    #[cfg(target_os = "windows")]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "hooks": [ { "type": "command", "command": "ping -n 6 127.0.0.1 >nul && echo late", "timeout": 1 } ] } ] } }"#;
    #[cfg(not(target_os = "windows"))]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "hooks": [ { "type": "command", "command": "sleep 5 && echo late", "timeout": 1 } ] } ] } }"#;

    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-timeout-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

#[tokio::test]
async fn fail_open_on_exit_2() {
    #[cfg(target_os = "windows")]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "cmd /c exit 2" } ] } }"#;
    #[cfg(not(target_os = "windows"))]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "exit 2" } ] } }"#;

    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-sess-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert!(get_hook_context(&sid).is_none());
    clear_hook_context(&sid);
}

/// Exec form (#3005): with `args` set, the command runs as an executable and
/// each element arrives verbatim. A shell would expand `$HOME` and split on
/// the `;`, so the literal output proves no shell ran.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn exec_form_args_bypass_the_shell() {
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "type": "command", "command": "printf", "args": ["%s|%s", "$HOME", "a;b c"] } ] } }"#;
    let (temp, root) = setup_approved_plugin(hooks_json);
    let sid = format!("hook-exec-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    run_session_start_in(temp.path(), &sid, SessionStartSource::Startup, Some(root)).await;
    assert_eq!(get_hook_context(&sid).as_deref(), Some("$HOME|a;b c"));
    clear_hook_context(&sid);
}

/// `updatedInput` (#3005): a `PreToolUse` hook that allows the call rewrites
/// the arguments, and the aggregated dispatch result carries the rewrite.
/// The handler carries a `powershell` variant (#3005 gatekeeper G3.1): an
/// unquoted `echo {...}` under `cmd.exe /C` arrives mangled, so Windows
/// writes the JSON through `Write-Output` like the neighbouring tests.
#[tokio::test]
async fn pre_tool_use_updated_input_reaches_the_dispatch_result() {
    let decision_json = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"path":"rewritten.txt"}}}"#;
    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("printf '%s' '{decision_json}'"),
                    "powershell": format!("Write-Output '{decision_json}'"),
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);
    let sid = format!("hook-updated-input-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    params.tool_name = Some("workspace__readFile".to_string());
    params.tool_input = Some(serde_json::json!({"path": "original.txt"}));

    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__readFile",
        &params,
        &sid,
    )
    .await;

    assert_eq!(
        outcome.decision,
        Some(crate::services::hooks::HookDecision::Allow)
    );
    assert_eq!(
        outcome.updated_input,
        Some(serde_json::json!({"path": "rewritten.txt"}))
    );
}

/// #3005 gatekeeper G1.3: two exec-form handlers with the same command
/// but different `args` are separate approvals and both run — sharing one
/// approval key would silently skip the second as "unapproved or
/// modified".
#[cfg(unix)]
#[tokio::test]
async fn same_command_different_args_both_run_through_dispatch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_id = "plugin-0123456789abcdef".to_string();
    let root = temp.path().join(&source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    // The scripts must exist before approval: the dispatch-time hash
    // check covers every script the handler references.
    for (name, marker) in [("a.sh", "marker-a.txt"), ("b.sh", "marker-b.txt")] {
        let path = root.join("hooks").join(name);
        fs::write(&path, format!("#!/bin/sh\ntouch {marker}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    fs::write(
        root.join("hooks").join("hooks.json"),
        r#"{ "hooks": { "PreToolUse": [ { "matcher": "workspace__readFile", "hooks": [
            { "type": "command", "command": "sh", "args": ["hooks/a.sh"] },
            { "type": "command", "command": "sh", "args": ["hooks/b.sh"] }
        ] } ] } }"#,
    )
    .unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
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
            approval_hashes: compute_handler_approval_hashes(&root, &cfg),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();

    let sid = format!("hook-args-keys-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    params.tool_name = Some("workspace__readFile".to_string());
    params.tool_input = Some(serde_json::json!({ "path": "x.txt" }));

    dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__readFile",
        &params,
        &sid,
    )
    .await;

    assert!(
        root.join("marker-a.txt").exists() && root.join("marker-b.txt").exists(),
        "both same-command different-args handlers must run; \
         a: {}, b: {}",
        root.join("marker-a.txt").exists(),
        root.join("marker-b.txt").exists()
    );
}

/// A `block` decision drops the rewrite: the call never runs, so there is
/// nothing for `updatedInput` to change (#3005).
#[test]
fn updated_input_from_a_blocking_handler_is_dropped() {
    let mut result = HookDispatchResult::default();
    result.merge(HookOutput {
        additional_context: None,
        decision: Some(crate::services::hooks::HookDecision::Block),
        reason: Some("denied".to_string()),
        continue_loop: None,
        updated_input: Some(serde_json::json!({"path": "changed.txt"})),
    });
    assert_eq!(result.updated_input, None);
}

/// The last allowing handler's rewrite wins, matching Claude Code's single
/// `updatedInput` per call (#3005).
#[test]
fn last_allow_rewrite_wins() {
    let mut result = HookDispatchResult::default();
    for path in ["first.txt", "second.txt"] {
        result.merge(HookOutput {
            additional_context: None,
            decision: Some(crate::services::hooks::HookDecision::Allow),
            reason: None,
            continue_loop: None,
            updated_input: Some(serde_json::json!({ "path": path })),
        });
    }
    assert_eq!(
        result.updated_input,
        Some(serde_json::json!({"path": "second.txt"}))
    );
}

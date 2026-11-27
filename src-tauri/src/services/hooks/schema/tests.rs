use super::parser::parse_hooks_value;
use super::*;
use std::fs;
use std::path::Path;

fn superpowers_claude_json() -> &'static str {
    r#"{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|clear|compact",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd session-start",
            "powershell": "& \"${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd\" session-start",
            "shell": "bash",
            "async": false
          }
        ]
      }
    ]
  }
}"#
}

#[test]
fn parses_superpowers_session_start() {
    let cfg = parse_hooks_value(
        &serde_json::from_str(superpowers_claude_json()).unwrap(),
        Path::new("test.json"),
    )
    .unwrap();
    let groups = cfg.events.get(&HookEvent::SessionStart).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].matcher.as_deref(), Some("startup|clear|compact"));
    let h = &groups[0].handlers[0];
    assert!(h.powershell.is_some());
    assert_eq!(
        h.timeout_ms,
        default_hook_timeout_ms(HookEvent::SessionStart)
    );
    assert_eq!(
        HookEvent::SessionStart.dispatch_mode(),
        HookDispatchMode::Execute
    );
    assert_eq!(
        HookEvent::PreToolUse.dispatch_mode(),
        HookDispatchMode::Execute
    );
}

#[test]
fn cursor_camel_case_normalizes_to_session_start() {
    let json = r#"{
  "version": 1,
  "hooks": {
    "sessionStart": [
      { "command": "./hooks/run-hook.cmd session-start" }
    ]
  }
}"#;
    let cfg = parse_hooks_value(
        &serde_json::from_str(json).unwrap(),
        Path::new("hooks-cursor.json"),
    )
    .unwrap();
    assert!(cfg.events.contains_key(&HookEvent::SessionStart));
}

#[test]
fn timeout_ms_alias_is_kept_verbatim() {
    let json = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "echo ok", "timeoutMs": 1500 }
        ]
      }
    ]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    assert_eq!(
        cfg.events[&HookEvent::SessionStart][0].handlers[0].timeout_ms,
        1500
    );
}

#[test]
fn default_timeouts_follow_the_claude_code_values() {
    let json = r#"{
  "hooks": {
    "PreToolUse": [{ "command": "echo pre" }],
    "UserPromptSubmit": [{ "command": "echo prompt" }],
    "SessionEnd": [{ "command": "echo end" }]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    let timeout = |event| cfg.events[&event][0].handlers[0].timeout_ms;
    assert_eq!(timeout(HookEvent::PreToolUse), 600_000);
    assert_eq!(timeout(HookEvent::UserPromptSubmit), 30_000);
    assert_eq!(timeout(HookEvent::SessionEnd), 1_500);
}

#[test]
fn explicit_timeout_seconds_win_over_the_event_default() {
    let json = r#"{
  "hooks": {
    "SessionEnd": [{ "command": "echo end", "timeout": 20 }]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    assert_eq!(
        cfg.events[&HookEvent::SessionEnd][0].handlers[0].timeout_ms,
        20_000
    );
}

#[test]
fn args_select_exec_form_and_are_kept_in_order() {
    let json = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/scripts/format.js",
            "args": ["--fix", "a file.txt"]
          }
        ]
      }
    ]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    assert_eq!(
        cfg.events[&HookEvent::PreToolUse][0].handlers[0]
            .args
            .as_deref(),
        Some(["--fix".to_string(), "a file.txt".to_string()].as_slice())
    );
}

#[test]
fn without_args_the_handler_stays_in_shell_form() {
    let json = r#"{
  "hooks": {
    "PreToolUse": [{ "command": "echo hi | tr a-z A-Z" }]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    assert!(cfg.events[&HookEvent::PreToolUse][0].handlers[0]
        .args
        .is_none());
}

#[test]
fn records_unknown_and_newer_events_as_unsupported_without_failing() {
    for key in ["Setup", "InstructionsLoaded", "bogusEvent"] {
        let json = format!(
            r#"{{"hooks":{{"{key}":[{{"matcher":"Bash","hooks":[{{"type":"command","command":"echo hi"}}]}}],"SessionStart":[{{"hooks":[{{"type":"command","command":"echo start"}}]}}]}}}}"#
        );
        let cfg = parse_hooks_value(&serde_json::from_str(&json).unwrap(), Path::new("t.json"))
            .expect("unknown/newer hook events should not fail install");
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
        let entries = cfg
            .unsupported_events
            .get(key)
            .expect("unsupported event should be recorded");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(entries[0].command, "echo hi");
    }
}

#[test]
fn parses_new_session_and_subagent_events_as_supported() {
    // #3007: SubagentStart, PostCompact, and StopFailure moved from the
    // reserved set into the supported event schema.
    for (key, event) in [
        ("SubagentStart", HookEvent::SubagentStart),
        ("subagentStart", HookEvent::SubagentStart),
        ("PostCompact", HookEvent::PostCompact),
        ("postCompact", HookEvent::PostCompact),
        ("StopFailure", HookEvent::StopFailure),
        ("stopFailure", HookEvent::StopFailure),
    ] {
        let json = format!(
            r#"{{"hooks":{{"{key}":[{{"matcher":"x","hooks":[{{"type":"command","command":"echo hi"}}]}}]}}}}"#
        );
        let cfg = parse_hooks_value(&serde_json::from_str(&json).unwrap(), Path::new("t.json"))
            .expect("supported hook events must parse");
        assert!(
            cfg.events.contains_key(&event),
            "{key} must parse as {event:?}"
        );
        assert!(cfg.unsupported_events.is_empty());
    }
}

#[test]
fn records_unsupported_handler_types_without_failing_install() {
    // #3009: prompt/mcp_tool/agent and unknown types on supported events
    // install and are recorded as unsupported instead of failing.
    for handler_type in ["prompt", "mcp_tool", "agent", "bogus"] {
        let json = format!(
            r#"{{"hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{{"type":"{handler_type}"}}]}}],"SessionStart":[{{"hooks":[{{"type":"command","command":"echo start"}}]}}]}}}}"#
        );
        let cfg = parse_hooks_value(&serde_json::from_str(&json).unwrap(), Path::new("t.json"))
            .expect("unsupported handler types must not fail install");
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
        assert!(
            cfg.events[&HookEvent::PreToolUse][0].handlers.is_empty(),
            "{handler_type} must not produce a runnable handler"
        );
        let entries = &cfg.unsupported_handlers[HookEvent::PreToolUse.as_str()];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].matcher.as_deref(), Some("Bash"));
        assert!(entries[0].command.contains(handler_type));
    }
}

#[test]
fn parses_http_handler_and_requires_url() {
    let json = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "http",
            "url": "http://localhost:8080/hooks/pre-tool-use",
            "timeout": 30,
            "headers": { "Authorization": "Bearer $MY_TOKEN" },
            "allowedEnvVars": ["MY_TOKEN"]
          }
        ]
      }
    ]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    let handler = &cfg.events[&HookEvent::PreToolUse][0].handlers[0];
    let http = handler.http.as_ref().expect("http config must be parsed");
    assert_eq!(http.url, "http://localhost:8080/hooks/pre-tool-use");
    assert_eq!(
        http.headers.get("Authorization").map(String::as_str),
        Some("Bearer $MY_TOKEN")
    );
    assert_eq!(http.allowed_env_vars, vec!["MY_TOKEN"]);
    assert_eq!(handler.timeout_ms, 30_000);
    assert!(cfg.unsupported_handlers.is_empty());

    let no_url = r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"http"}]}]}}"#;
    assert!(
        parse_hooks_value(&serde_json::from_str(no_url).unwrap(), Path::new("t.json"))
            .unwrap_err()
            .contains("url")
    );
}

/// Non-http(s) or unparsable URLs must fail at install/parse (#3041
/// G1.5), not silently fail the hook on every event at runtime.
#[test]
fn rejects_invalid_http_handler_url_at_parse_time() {
    for url in ["ftp://example.com/hook", "not a url", "file:///etc/passwd"] {
        let json = serde_json::json!({
            "hooks": { "PreToolUse": [{ "hooks": [{ "type": "http", "url": url }] }] }
        });
        let error = parse_hooks_value(&json, Path::new("t.json"))
            .expect_err("{url:?} must be rejected at parse time");
        assert!(
            error.contains("url"),
            "error should name the offending field, got: {error}"
        );
    }
}

#[test]
fn rejects_zero_timeout() {
    let bad_timeout =
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x","timeout":0}]}]}}"#;
    assert!(parse_hooks_value(
        &serde_json::from_str(bad_timeout).unwrap(),
        Path::new("t.json")
    )
    .unwrap_err()
    .contains("timeout"));
}

#[test]
fn rejects_session_start_async_true() {
    let json = r#"{
  "hooks": {
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "x", "async": true } ] }
    ]
  }
}"#;
    let err =
        parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap_err();
    assert!(err.contains("async"));
}

#[test]
fn parses_tool_lifecycle_events_as_supported() {
    let json = r#"{
  "hooks": {
    "PostToolUseFailure": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo fail" }] }],
    "PermissionRequest": [{ "matcher": "workspace__runShell", "hooks": [{ "type": "command", "command": "echo perm" }] }],
    "PermissionDenied": [{ "matcher": "*", "hooks": [{ "type": "command", "command": "echo denied" }] }],
    "PostToolBatch": [{ "hooks": [{ "type": "command", "command": "echo batch" }] }]
  }
}"#;
    let cfg = parse_hooks_value(&serde_json::from_str(json).unwrap(), Path::new("t.json")).unwrap();
    assert!(cfg.unsupported_events.is_empty());
    assert!(cfg.events.contains_key(&HookEvent::PostToolUseFailure));
    assert!(cfg.events.contains_key(&HookEvent::PermissionRequest));
    assert!(cfg.events.contains_key(&HookEvent::PermissionDenied));
    assert!(cfg.events.contains_key(&HookEvent::PostToolBatch));
}

#[test]
fn load_from_root_merges_claude_and_cursor_files() {
    let dir = tempfile::tempdir().unwrap();
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(hooks_dir.join("hooks.json"), superpowers_claude_json()).unwrap();
    fs::write(
        hooks_dir.join("hooks-cursor.json"),
        r#"{"hooks":{"sessionEnd":[{"command":"echo bye"}]}}"#,
    )
    .unwrap();

    let cfg = load_hooks_from_root(dir.path()).unwrap();
    assert!(cfg.events.contains_key(&HookEvent::SessionStart));
    assert!(cfg.events.contains_key(&HookEvent::SessionEnd));
}

#[test]
fn lifecycle_events_are_supported_and_the_rest_record_why_not() {
    let supported = [
        "UserPromptExpansion",
        "MessageDisplay",
        "InstructionsLoaded",
        "ConfigChange",
        "CwdChanged",
        "PreModelSwitch",
        "PostModelSwitch",
    ];
    let not_emitted = [
        "PreSkillShellCommand",
        "TeammateIdle",
        "DirectoryAdded",
        "FileChanged",
        "WorktreeCreate",
        "WorktreeRemove",
        "Elicitation",
        "ElicitationResult",
        "TaskCreated",
        "TaskCompleted",
    ];
    let hooks: serde_json::Map<String, serde_json::Value> = supported
        .iter()
        .chain(not_emitted.iter())
        .map(|name| {
            // No command, so an unsupported entry carries the recorded note.
            let group = if supported.contains(name) {
                serde_json::json!({ "command": "true" })
            } else {
                serde_json::json!({ "hooks": [] })
            };
            (name.to_string(), serde_json::json!([group]))
        })
        .collect();
    let cfg = parse_hooks_inline(&serde_json::Value::Object(hooks), "test").expect("parse");

    let names: Vec<&str> = cfg.events.keys().map(|event| event.as_str()).collect();
    assert_eq!(names, supported);
    assert_eq!(cfg.unsupported_events.len(), not_emitted.len());
    for name in not_emitted {
        let note = &cfg.unsupported_events[name][0].command;
        assert!(
            note.contains(&format!("does not emit {name}")),
            "{name}: {note}"
        );
    }
    assert_eq!(default_hook_timeout_ms(HookEvent::MessageDisplay), 10_000);
    assert_eq!(default_hook_timeout_ms(HookEvent::PreModelSwitch), 30_000);
    assert_eq!(default_hook_timeout_ms(HookEvent::ConfigChange), 600_000);
}

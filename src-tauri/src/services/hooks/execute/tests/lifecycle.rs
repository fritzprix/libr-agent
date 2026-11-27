//! The #3008 events through their real lifecycle functions and an approved
//! plugin: stdin fields, matchers, and the output each event accepts.

use super::*;
use crate::services::hooks::context::take_turn_hook_context;
use crate::services::hooks::lifecycle::{
    config_change, cwd_changed, instructions_loaded, message_display, note_session_start,
    post_model_switch, pre_model_switch, take_instruction_reload, user_prompt_expansion,
    HookSources, ModelSwitch,
};
use serde_json::{json, Value};

fn sources(temp: &Path) -> HookSources {
    HookSources {
        plugins_dir: temp.to_path_buf(),
        session_plugins_dir: None,
        session_trust_dir: None,
    }
}

fn session(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}

/// One handler on `event` running the given shell command (`sh`, `powershell`).
fn plugin(event: &str, matcher: Option<&str>, sh: &str, ps: &str) -> (TempDir, PathBuf) {
    let mut group = json!({
        "hooks": [{ "type": "command", "command": sh, "powershell": ps }]
    });
    if let Some(matcher) = matcher {
        group["matcher"] = json!(matcher);
    }
    setup_approved_plugin(&json!({ "hooks": { event: [group] } }).to_string())
}

/// A handler that appends its stdin to `log`.
fn recorder(event: &str, matcher: Option<&str>, log: &Path) -> (TempDir, PathBuf) {
    let log = log.to_string_lossy();
    plugin(
        event,
        matcher,
        &format!("{{ cat; echo; }} >> '{log}'"),
        &format!("[Console]::In.ReadToEnd() | Add-Content -Path '{log}'"),
    )
}

fn printing(event: &str, matcher: Option<&str>, stdout: &str) -> (TempDir, PathBuf) {
    plugin(
        event,
        matcher,
        &format!("printf '%s' '{stdout}'"),
        &format!("Write-Output '{stdout}'"),
    )
}

fn recorded(log: &Path) -> Vec<Value> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line.trim_start_matches('\u{feff}')).expect("stdin JSON"))
        .collect()
}

fn switch(to: &str) -> ModelSwitch {
    ModelSwitch {
        from_model: "model-a".to_string(),
        to_model: to.to_string(),
        source: "picker",
        context_tokens: Some(42),
    }
}

#[tokio::test]
async fn user_prompt_expansion_sends_command_fields_and_adds_context() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let (temp, _root) = recorder("UserPromptExpansion", Some("demo:review"), &log);
    let sid = session("expansion");
    for name in ["demo:review", "demo:other"] {
        user_prompt_expansion(&sources(temp.path()), &sid, name, "a b", "/review a b")
            .await
            .expect("not blocked");
    }
    let calls = recorded(&log);
    assert_eq!(calls.len(), 1, "the matcher selects by command_name");
    assert_eq!(calls[0]["hook_event_name"], "UserPromptExpansion");
    assert_eq!(calls[0]["expansion_type"], "slash_command");
    assert_eq!(calls[0]["command_name"], "demo:review");
    assert_eq!(calls[0]["command_args"], "a b");
    assert_eq!(calls[0]["command_source"], "plugin");
    assert_eq!(calls[0]["prompt"], "/review a b");

    let (temp, _root) = printing("UserPromptExpansion", None, "plain context");
    let sid = session("expansion-context");
    user_prompt_expansion(&sources(temp.path()), &sid, "demo:review", "", "/review")
        .await
        .expect("not blocked");
    assert_eq!(
        take_turn_hook_context(&sid).as_deref(),
        Some("plain context")
    );
}

#[tokio::test]
async fn user_prompt_expansion_blocks_on_decision_and_exit_2() {
    let (temp, _root) = printing(
        "UserPromptExpansion",
        None,
        r#"{"decision":"block","reason":"not here"}"#,
    );
    let blocked =
        user_prompt_expansion(&sources(temp.path()), &session("b"), "d:c", "", "/c").await;
    assert_eq!(blocked, Err("not here".to_string()));

    let (temp, _root) = plugin(
        "UserPromptExpansion",
        None,
        "echo denied >&2; exit 2",
        "[Console]::Error.WriteLine('denied'); exit 2",
    );
    let blocked =
        user_prompt_expansion(&sources(temp.path()), &session("b"), "d:c", "", "/c").await;
    assert_eq!(blocked, Err("denied".to_string()));
}

#[tokio::test]
async fn message_display_returns_display_content_and_ignores_matcher() {
    let (temp, _root) = printing(
        "MessageDisplay",
        Some("never-matches"),
        r#"{"hookSpecificOutput":{"displayContent":"shown"}}"#,
    );
    let shown = message_display(&sources(temp.path()), &session("md"), "t", "m", "original").await;
    assert_eq!(shown.as_deref(), Some("shown"));

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let (temp, _root) = recorder("MessageDisplay", None, &log);
    let shown = message_display(&sources(temp.path()), &session("md"), "t1", "m1", "text").await;
    assert_eq!(shown, None);
    let calls = recorded(&log);
    assert_eq!(calls[0]["turn_id"], "t1");
    assert_eq!(calls[0]["message_id"], "m1");
    assert_eq!(calls[0]["index"], 0);
    assert_eq!(calls[0]["final"], true);
    assert_eq!(calls[0]["delta"], "text");
}

#[tokio::test]
async fn instructions_loaded_fires_once_per_context_with_its_load_reason() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let (temp, _root) = recorder("InstructionsLoaded", None, &log);
    let sources = sources(temp.path());
    let sid = session("instructions");
    let file = dir.path().join("CLAUDE.md");

    instructions_loaded(&sources, &sid, &file).await;
    instructions_loaded(&sources, &sid, &file).await;
    note_session_start(&sid, SessionStartSource::Resume);
    assert!(!take_instruction_reload(&sid));
    note_session_start(&sid, SessionStartSource::Compact);
    assert!(take_instruction_reload(&sid));
    assert!(!take_instruction_reload(&sid));
    instructions_loaded(&sources, &sid, &file).await;

    let calls = recorded(&log);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["load_reason"], "session_start");
    assert_eq!(calls[0]["memory_type"], "Project");
    assert_eq!(calls[0]["file_path"], file.to_string_lossy().as_ref());
    assert_eq!(calls[1]["load_reason"], "compact");
}

#[tokio::test]
async fn config_change_blocks_only_on_a_block_decision() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let (temp, _root) = recorder("ConfigChange", Some("user_settings"), &log);
    assert_eq!(config_change(&sources(temp.path())).await, Ok(()));
    assert_eq!(recorded(&log)[0]["source"], "user_settings");

    let (temp, _root) = printing("ConfigChange", None, r#"{"continue":false}"#);
    assert_eq!(config_change(&sources(temp.path())).await, Ok(()));

    let (temp, _root) = printing(
        "ConfigChange",
        None,
        r#"{"decision":"block","reason":"secret"}"#,
    );
    let error = config_change(&sources(temp.path()))
        .await
        .expect_err("blocked");
    assert!(!error.contains("secret"), "the reason is never shown");
}

#[tokio::test]
async fn cwd_changed_sends_directories_and_applies_its_env_file() {
    let sid = session("cwd");
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let log_str = log.to_string_lossy();
    let (temp, _root) = plugin(
        "CwdChanged",
        Some("never-matches"),
        &format!("{{ cat; echo; }} >> '{log_str}'; echo DIR_VAR=on >> \"$CLAUDE_ENV_FILE\""),
        &format!("[Console]::In.ReadToEnd() | Add-Content -Path '{log_str}'; Add-Content -Path $env:CLAUDE_ENV_FILE -Value 'DIR_VAR=on'"),
    );
    cwd_changed(&sources(temp.path()), &sid, "/old", "/new").await;
    let calls = recorded(&log);
    assert_eq!(calls[0]["old_cwd"], "/old");
    assert_eq!(calls[0]["new_cwd"], "/new");
    assert_eq!(
        crate::services::hooks::plugin_env::session_plugin_env(&sid)
            .get("DIR_VAR")
            .map(String::as_str),
        Some("on")
    );
    crate::services::hooks::plugin_env::clear_session_plugin_env(&sid);
}

#[tokio::test]
async fn pre_model_switch_matches_the_target_and_can_cancel() {
    let (temp, _root) = printing(
        "PreModelSwitch",
        Some("model-b"),
        r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"too big"}}"#,
    );
    let sources_b = sources(temp.path());
    let error = pre_model_switch(&sources_b, &session("pre"), &switch("model-b"))
        .await
        .expect_err("denied");
    assert!(error.ends_with(": too big"), "{error}");
    assert_eq!(
        pre_model_switch(&sources_b, &session("pre"), &switch("model-c")).await,
        Ok(())
    );

    let (temp, _root) = printing(
        "PreModelSwitch",
        None,
        r#"{"hookSpecificOutput":{"permissionDecision":"ask"}}"#,
    );
    assert!(
        pre_model_switch(&sources(temp.path()), &session("pre"), &switch("model-b"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn pre_model_switch_timeout_cancels_the_switch() {
    let hooks = json!({ "hooks": { "PreModelSwitch": [{ "hooks": [{
        "type": "command",
        "command": "sleep 30",
        "powershell": "Start-Sleep -Seconds 30",
        "timeout": 1,
    }] }] } });
    let (temp, _root) = setup_approved_plugin(&hooks.to_string());
    assert!(
        pre_model_switch(&sources(temp.path()), &session("pre"), &switch("model-b"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn post_model_switch_sends_switch_fields_and_adds_context() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("stdin");
    let (temp, _root) = recorder("PostModelSwitch", None, &log);
    post_model_switch(&sources(temp.path()), &session("post"), &switch("model-b")).await;
    let calls = recorded(&log);
    assert_eq!(calls[0]["from_model"], "model-a");
    assert_eq!(calls[0]["to_model"], "model-b");
    assert_eq!(calls[0]["requested_model"], "model-b");
    assert_eq!(calls[0]["source"], "picker");
    assert_eq!(calls[0]["context_tokens"], 42);

    let (temp, _root) = printing("PostModelSwitch", None, "use short answers");
    let sid = session("post-context");
    post_model_switch(&sources(temp.path()), &sid, &switch("model-b")).await;
    assert_eq!(
        take_turn_hook_context(&sid).as_deref(),
        Some("use short answers")
    );
}

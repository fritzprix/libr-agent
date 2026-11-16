use super::context::{
    append_hook_context, begin_hook_replacement, begin_hook_run, end_hook_run,
    finish_hook_replacement,
};
use super::envelope::{build_envelope_value, HookEnvelopeParams, SessionStartSource};
use super::output::{normalize, HookOutput};
use super::registry::{
    approval_hash_key, approval_hash_value, get_session_plugins_directory,
    get_user_plugins_directory, list_effective_approved_sources_in,
};
use super::schema::{HookDispatchMode, HookEvent, HookHandler, DEFAULT_HOOK_TIMEOUT_SECS};
use crate::session::{get_session_manager, resolve_session_workspace_dir};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

pub async fn run_session_start(session_id: &str, source: SessionStartSource, cwd: Option<PathBuf>) {
    let plugins_dir = match get_user_plugins_directory() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!(
                "SessionStart hooks skipped for {}: plugins dir unavailable: {e}",
                session_id
            );
            return;
        }
    };
    let session_plugins_dir = get_session_plugins_directory(session_id).ok();
    run_session_start_with_sources(
        &plugins_dir,
        session_plugins_dir.as_deref(),
        session_id,
        source,
        cwd,
    )
    .await;
}

/// Releases the in-flight marker for a SessionStart hook run when the task
/// finishes — including on panic and early return — so `await_hook_run` can
/// never hang a completion past the hook run (#2812 G3.1).
struct HookRunGuard(String);

impl Drop for HookRunGuard {
    fn drop(&mut self) {
        end_hook_run(&self.0);
    }
}

/// Spawn the SessionStart hooks for a session off the caller's path.
///
/// Registers the in-flight marker synchronously before spawning, so the
/// first LLM request of a new session sees the wait even when it arrives
/// before the spawned task starts (#2812 G3.1).
pub fn spawn_session_start(session_id: &str, source: SessionStartSource) {
    begin_hook_run(session_id);
    let hook_session_id = session_id.to_string();
    tauri::async_runtime::spawn(async move {
        let _guard = HookRunGuard(hook_session_id.clone());
        run_session_start(&hook_session_id, source, None).await;
    });
}

pub async fn run_session_start_in(
    plugins_dir: &Path,
    session_id: &str,
    source: SessionStartSource,
    cwd: Option<PathBuf>,
) {
    run_session_start_with_sources(plugins_dir, None, session_id, source, cwd).await;
}

/// Run SessionStart hooks from the global plugin store plus an optional
/// session-scoped store (session installs shadow global ones, #2825).
pub async fn run_session_start_with_sources(
    plugins_dir: &Path,
    session_plugins_dir: Option<&Path>,
    session_id: &str,
    source: SessionStartSource,
    cwd: Option<PathBuf>,
) {
    // Do NOT clear the previous context up front (#2812 G3.2): a slow,
    // failing, or exit-2 re-run must not erase a bootstrap that is already
    // in effect. Arm one-shot replacement instead — the first output of
    // this run replaces the stored text; without output, finish disarms
    // and the previous text survives.
    begin_hook_replacement(session_id);
    let cwd = match resolve_session_cwd(session_id, cwd).await {
        Ok(p) => p,
        Err(e) => {
            log::warn!(
                "SessionStart hooks skipped for {}: workspace cwd unavailable: {e}",
                session_id
            );
            finish_hook_replacement(session_id);
            return;
        }
    };
    let matcher_value = session_start_source_label(source);
    let mut params = HookEnvelopeParams::new(session_id, cwd, HookEvent::SessionStart);
    params.session_start_source = Some(source);
    params.matcher = Some(matcher_value.to_string());

    dispatch_hook_event_with_sources(
        plugins_dir,
        session_plugins_dir,
        HookEvent::SessionStart,
        matcher_value,
        &params,
        session_id,
    )
    .await;
    finish_hook_replacement(session_id);
}

pub async fn notify_stub_event(event: HookEvent, session_id: &str, mut params: HookEnvelopeParams) {
    if event.dispatch_mode() != HookDispatchMode::Stub {
        return;
    }
    params.event = event;
    params.session_id = session_id.to_string();
    if let Ok(cwd) = resolve_session_cwd(session_id, None).await {
        params.cwd = cwd;
    }
    log::info!(
        "plugin hook {:?} for session {} not yet executed (stub)",
        event,
        session_id
    );
    let _ = build_envelope_value(&params);
    log::debug!(
        "plugin hook stub envelope prepared for {:?} session={}",
        event,
        session_id
    );
}

pub async fn dispatch_hook_event(
    event: HookEvent,
    matcher_value: &str,
    envelope_params: &HookEnvelopeParams,
    session_id: &str,
) {
    let plugins_dir = match get_user_plugins_directory() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!(
                "plugin hook {} skipped for {}: plugins dir unavailable: {e}",
                event.as_str(),
                session_id
            );
            return;
        }
    };
    let session_plugins_dir = get_session_plugins_directory(session_id).ok();
    dispatch_hook_event_with_sources(
        &plugins_dir,
        session_plugins_dir.as_deref(),
        event,
        matcher_value,
        envelope_params,
        session_id,
    )
    .await;
}

pub async fn dispatch_hook_event_in(
    plugins_dir: &Path,
    event: HookEvent,
    matcher_value: &str,
    envelope_params: &HookEnvelopeParams,
    session_id: &str,
) {
    dispatch_hook_event_with_sources(
        plugins_dir,
        None,
        event,
        matcher_value,
        envelope_params,
        session_id,
    )
    .await;
}

pub async fn dispatch_hook_event_with_sources(
    plugins_dir: &Path,
    session_plugins_dir: Option<&Path>,
    event: HookEvent,
    matcher_value: &str,
    envelope_params: &HookEnvelopeParams,
    session_id: &str,
) {
    match event.dispatch_mode() {
        HookDispatchMode::Stub => {
            log::info!(
                "plugin hook {} not yet executed (stub) for session {}",
                event.as_str(),
                session_id
            );
            return;
        }
        HookDispatchMode::Execute => {}
    }

    let envelope = build_envelope_value(envelope_params);
    let envelope_json = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "SessionStart hooks skipped for {}: envelope JSON encode failed: {e}",
                session_id
            );
            return;
        }
    };

    for record in list_effective_approved_sources_in(plugins_dir, session_plugins_dir) {
        let root = &record.root_dir;
        let config = match super::schema::load_hooks_from_root(root) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("skip plugin hooks under {}: {}", root.display(), e);
                continue;
            }
        };
        let Some(groups) = config.events.get(&event) else {
            continue;
        };
        for group in groups {
            if !matcher_matches(group.matcher.as_deref(), matcher_value) {
                continue;
            }
            for handler in &group.handlers {
                let key = approval_hash_key(event, group.matcher.as_deref(), handler);
                let expected = approval_hash_value(root, handler);
                if record.manifest.approval_hashes.get(&key) != Some(&expected) {
                    log::warn!(
                        "skipping unapproved or modified hook handler in {} (source_id={})",
                        root.display(),
                        record.manifest.source_id
                    );
                    continue;
                }
                run_matching_handler(session_id, root, handler, &envelope_json).await;
            }
        }
    }
}

async fn run_matching_handler(
    session_id: &str,
    plugin_root: &Path,
    handler: &HookHandler,
    envelope_json: &str,
) {
    match run_hook_handler(plugin_root, handler, envelope_json).await {
        HookRunOutcome::Applied(output) => {
            if let Some(ctx) = output
                .additional_context
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                append_hook_context(session_id, ctx);
            }
        }
        HookRunOutcome::Skipped(reason) => {
            log::debug!(
                "hook handler skipped for session {}: {}",
                session_id,
                reason
            );
        }
    }
}

enum HookRunOutcome {
    Applied(HookOutput),
    Skipped(String),
}

async fn run_hook_handler(
    plugin_root: &Path,
    handler: &HookHandler,
    envelope_json: &str,
) -> HookRunOutcome {
    let timeout_secs = if handler.timeout_secs == 0 {
        DEFAULT_HOOK_TIMEOUT_SECS
    } else {
        handler.timeout_secs
    };
    let deadline = Duration::from_secs(timeout_secs);

    let mut child = match build_hook_command(plugin_root, handler) {
        Ok(mut cmd) => match cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return HookRunOutcome::Skipped(format!("spawn failed: {e}"));
            }
        },
        Err(e) => return HookRunOutcome::Skipped(e),
    };

    let pid = child.id();
    let mut stdin = child.stdin.take();
    let envelope_bytes = envelope_json.as_bytes().to_vec();
    let run_future = async {
        if let Some(mut pipe) = stdin.take() {
            if let Err(e) = pipe.write_all(&envelope_bytes).await {
                return Err(format!("stdin write failed: {e}"));
            }
        }
        child
            .wait_with_output()
            .await
            .map_err(|e| format!("wait failed: {e}"))
    };

    let output = match timeout(deadline, run_future).await {
        Ok(Ok(out)) => out,
        Ok(Err(reason)) => return HookRunOutcome::Skipped(reason),
        Err(_) => {
            if let Some(pid) = pid {
                let _ = tokio::task::spawn_blocking(move || {
                    crate::utils::process::force_kill_process_tree(pid)
                })
                .await;
            }
            return HookRunOutcome::Skipped("hook timed out".to_string());
        }
    };

    let status = output.status;
    let code = status.code().unwrap_or(-1);
    if code == 2 {
        return HookRunOutcome::Skipped("hook exit 2 (fail-open)".to_string());
    }
    if !status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return HookRunOutcome::Skipped(format!("hook exit {code}: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = normalize(&stdout, HookEvent::SessionStart);
    HookRunOutcome::Applied(parsed)
}

fn build_hook_command(plugin_root: &Path, handler: &HookHandler) -> Result<Command, String> {
    let root_str = plugin_root.to_string_lossy();
    #[cfg(target_os = "windows")]
    {
        if let Some(ps) = handler.powershell.as_deref() {
            let script = expand_plugin_root_placeholders(ps, plugin_root);
            let mut cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-Command", &script]);
            cmd.env("LIBRAGENT_PLUGIN_ROOT", root_str.as_ref());
            cmd.env("CLAUDE_PLUGIN_ROOT", root_str.as_ref());
            cmd.env("CURSOR_PLUGIN_ROOT", root_str.as_ref());
            cmd.current_dir(plugin_root);
            return Ok(cmd);
        }
        let script = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/C", &script]);
        cmd.env("LIBRAGENT_PLUGIN_ROOT", root_str.as_ref());
        cmd.env("CLAUDE_PLUGIN_ROOT", root_str.as_ref());
        cmd.env("CURSOR_PLUGIN_ROOT", root_str.as_ref());
        cmd.current_dir(plugin_root);
        Ok(cmd)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let shell = handler.shell.as_deref().unwrap_or("bash");
        let mut cmd = if shell == "sh" {
            let mut c = Command::new("sh");
            c.args(["-c", &script]);
            c
        } else {
            let mut c = Command::new("bash");
            c.args(["-c", &script]);
            c
        };
        cmd.env("LIBRAGENT_PLUGIN_ROOT", root_str.as_ref());
        cmd.env("CLAUDE_PLUGIN_ROOT", root_str.as_ref());
        cmd.env("CURSOR_PLUGIN_ROOT", root_str.as_ref());
        cmd.current_dir(plugin_root);
        Ok(cmd)
    }
}

fn expand_plugin_root_placeholders(command: &str, plugin_root: &Path) -> String {
    let root = plugin_root.to_string_lossy();
    command
        .replace("${LIBRAGENT_PLUGIN_ROOT}", &root)
        .replace("${CLAUDE_PLUGIN_ROOT}", &root)
        .replace("${CURSOR_PLUGIN_ROOT}", &root)
        .replace("$LIBRAGENT_PLUGIN_ROOT", &root)
        .replace("$CLAUDE_PLUGIN_ROOT", &root)
        .replace("$CURSOR_PLUGIN_ROOT", &root)
}

pub fn matcher_matches(matcher: Option<&str>, value: &str) -> bool {
    let Some(pattern) = matcher.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(e) => {
            log::warn!("invalid hook matcher regex {:?}: {e}", pattern);
            false
        }
    }
}

fn session_start_source_label(source: SessionStartSource) -> &'static str {
    match source {
        SessionStartSource::Startup => "startup",
        SessionStartSource::Resume => "resume",
        SessionStartSource::Clear => "clear",
        SessionStartSource::Compact => "compact",
    }
}

async fn resolve_session_cwd(
    session_id: &str,
    override_cwd: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(p) = override_cwd {
        return Ok(p);
    }
    let session_manager = get_session_manager()?;
    resolve_session_workspace_dir(session_manager, session_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hooks::context::{await_hook_run, clear_hook_context, get_hook_context};
    use crate::services::hooks::registry::{
        compute_handler_approval_hashes, save_source_in, PluginSourceManifest,
    };
    use crate::services::hooks::schema::load_hooks_from_root;
    use std::fs;
    use tempfile::TempDir;

    /// Rewrite a source's hooks.json and re-approve it, simulating a plugin
    /// update between two SessionStart runs of the same session.
    fn reapprove_source(temp: &Path, root: &Path, hooks_json: &str) {
        fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
        let source_id = root.file_name().unwrap().to_string_lossy().to_string();
        let cfg = load_hooks_from_root(root).unwrap();
        let hashes = compute_handler_approval_hashes(root, &cfg);
        save_source_in(
            temp,
            PluginSourceManifest {
                source_url: "test://local".to_string(),
                source_id,
                skill_names: vec![],
                hooks: None,
                hooks_approved: true,
                approval_hashes: hashes,
                created_at: 0,
            },
        )
        .unwrap();
    }

    fn setup_approved_plugin(hooks_json: &str) -> (TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_id = "plugin-0123456789abcdef".to_string();
        let root = temp.path().join(&source_id);
        fs::create_dir_all(root.join("hooks")).unwrap();
        fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
        let cfg = load_hooks_from_root(&root).unwrap();
        let hashes = compute_handler_approval_hashes(&root, &cfg);
        save_source_in(
            temp.path(),
            PluginSourceManifest {
                source_url: "test://local".to_string(),
                source_id,
                skill_names: vec![],
                hooks: None,
                hooks_approved: true,
                approval_hashes: hashes,
                created_at: 0,
            },
        )
        .unwrap();
        (temp, root)
    }

    #[test]
    fn matcher_or_patterns() {
        assert!(matcher_matches(Some("startup|clear"), "startup"));
        assert!(!matcher_matches(Some("startup|clear"), "resume"));
        assert!(matcher_matches(None, "anything"));
    }

    #[test]
    fn stub_dispatch_mode_does_not_run_handlers() {
        assert_eq!(
            HookEvent::PreToolUse.dispatch_mode(),
            HookDispatchMode::Stub
        );
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
                    source_url: "test://local".to_string(),
                    source_id,
                    skill_names: vec![],
                    hooks: None,
                    hooks_approved: true,
                    approval_hashes: hashes,
                    created_at: 0,
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

        let updated =
            r#"{ "hooks": { "SessionStart": [ { "command": "echo second bootstrap" } ] } }"#;
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
        let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "sleep 0.4 && echo slow hook body" } ] } }"#;

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
                source_url: "test://local".to_string(),
                source_id,
                skill_names: vec![],
                hooks: None,
                hooks_approved: true,
                approval_hashes: hashes,
                created_at: 0,
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

    #[tokio::test]
    async fn stub_event_logs_without_spawning() {
        let marker_dir = tempfile::tempdir().expect("marker dir");
        let marker = marker_dir.path().join("spawned.txt");
        let marker_str = marker.to_string_lossy().replace('\\', "\\\\");
        let hooks_json = format!(
            r#"{{ "hooks": {{ "PreToolUse": [ {{ "command": "echo spawned > \"{}\"" }} ] }} }}"#,
            marker_str
        );
        let (temp, root) = setup_approved_plugin(&hooks_json);

        let sid = "stub-session";
        let params = HookEnvelopeParams::new(sid, root, HookEvent::PreToolUse);
        dispatch_hook_event_in(temp.path(), HookEvent::PreToolUse, "Bash", &params, sid).await;
        notify_stub_event(HookEvent::PreToolUse, sid, params).await;

        assert!(!marker.exists(), "stub event must not spawn a process");
        clear_hook_context(sid);
    }

    /// Criterion 7: a source that declares a stub event with a command that
    /// would write a marker must not create that file when routed through
    /// `notify_stub_event` (no spawn, no stdout apply).
    #[tokio::test]
    async fn notify_stub_event_does_not_run_declared_handler() {
        let marker_dir = tempfile::tempdir().expect("marker dir");
        let marker = marker_dir.path().join("session-end-ran.txt");
        let marker_str = marker.to_string_lossy().replace('\\', "\\\\");
        #[cfg(windows)]
        let command = format!("cmd /c echo ran> \"{}\"", marker_str);
        #[cfg(not(windows))]
        let command = format!("sh -c 'echo ran > \"{}\"'", marker_str);
        let hooks_json = format!(
            r#"{{ "hooks": {{ "SessionEnd": [ {{ "command": "{}" }} ] }} }}"#,
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let (_temp, root) = setup_approved_plugin(&hooks_json);

        let sid = format!("stub-session-end-{}", uuid::Uuid::new_v4());
        let params = HookEnvelopeParams::new(&sid, root, HookEvent::SessionEnd);
        notify_stub_event(HookEvent::SessionEnd, &sid, params).await;

        assert!(
            !marker.exists(),
            "notify_stub_event must not spawn the declared SessionEnd command"
        );
        assert_eq!(
            HookEvent::SessionEnd.dispatch_mode(),
            HookDispatchMode::Stub
        );
        clear_hook_context(&sid);
    }
    // --- #2825 session-scoped plugin sources ---------------------------------

    fn write_source(base: &Path, source_id: &str, echo: &str, approved: bool) -> PathBuf {
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
                source_url: format!("test://{echo}"),
                source_id: source_id.to_string(),
                skill_names: vec![],
                hooks: None,
                hooks_approved: approved,
                approval_hashes: hashes,
                created_at: 0,
            },
        )
        .unwrap();
        root
    }

    async fn run_with_sources(global: &Path, session: Option<&Path>) -> Option<String> {
        let sid = format!("hook-scope-{}", uuid::Uuid::new_v4());
        clear_hook_context(&sid);
        run_session_start_with_sources(
            global,
            session,
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
        let id = "plugin-aaaaaaaaaaaaaaaa";
        write_source(global.path(), id, "from-global", true);
        write_source(session.path(), id, "from-session", true);

        let out = run_with_sources(global.path(), Some(session.path()))
            .await
            .expect("hook output");
        assert_eq!(out, "from-session", "hooks must run once, session wins");
    }

    #[tokio::test]
    async fn unapproved_session_install_still_suppresses_global_copy() {
        let global = tempfile::tempdir().unwrap();
        let session = tempfile::tempdir().unwrap();
        let id = "plugin-bbbbbbbbbbbbbbbb";
        write_source(global.path(), id, "from-global", true);
        write_source(session.path(), id, "from-session", false);

        let out = run_with_sources(global.path(), Some(session.path())).await;
        assert!(
            out.as_deref().is_none_or(|text| text.trim().is_empty()),
            "global hooks must stay suppressed in this session: {out:?}"
        );
    }

    #[tokio::test]
    async fn session_plugin_does_not_leak_into_other_sessions() {
        let global = tempfile::tempdir().unwrap();
        let session_a = tempfile::tempdir().unwrap();
        let session_b = tempfile::tempdir().unwrap();
        write_source(
            global.path(),
            "plugin-cccccccccccccccc",
            "global-hook",
            true,
        );
        write_source(
            session_a.path(),
            "plugin-dddddddddddddddd",
            "session-a-hook",
            true,
        );

        let a = run_with_sources(global.path(), Some(session_a.path()))
            .await
            .expect("session A output");
        let b = run_with_sources(global.path(), Some(session_b.path()))
            .await
            .expect("session B output");
        assert!(a.contains("global-hook") && a.contains("session-a-hook"));
        assert_eq!(
            b, "global-hook",
            "session B must only see the global plugin"
        );
    }
}

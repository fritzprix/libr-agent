use super::context::{
    append_hook_context, begin_hook_replacement, begin_hook_run, end_hook_run,
    finish_hook_replacement,
};
use super::envelope::{build_envelope_value, HookEnvelopeParams, SessionStartSource};
use super::output::{normalize, HookDecision, HookOutput};
use super::registry::{
    approval_hash_key, approval_hash_value, get_session_plugin_trust_dir,
    get_session_plugins_directory, get_user_plugins_directory, list_effective_approved_sources_in,
};
use super::schema::{
    HookDispatchMode, HookEvent, HookHandler, HookHttpConfig, DEFAULT_HOOK_TIMEOUT_SECS,
};
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
    let session_trust_dir = get_session_plugin_trust_dir(session_id).ok();
    run_session_start_with_sources(
        &plugins_dir,
        session_plugins_dir.as_deref(),
        session_trust_dir.as_deref(),
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
    run_session_start_with_sources(plugins_dir, None, None, session_id, source, cwd).await;
}

/// Run SessionStart hooks from the global plugin store plus an optional
/// session-scoped store (session installs shadow global ones, #2825).
/// `session_trust_dir` is the out-of-workspace trust store for that session
/// store (#2942 G1.1); `None` means session plugins load unapproved.
pub async fn run_session_start_with_sources(
    plugins_dir: &Path,
    session_plugins_dir: Option<&Path>,
    session_trust_dir: Option<&Path>,
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
        session_trust_dir,
        HookEvent::SessionStart,
        matcher_value,
        &params,
        session_id,
    )
    .await;
    finish_hook_replacement(session_id);
}

/// Aggregated result of running all matching approved hook handlers for one event (#2925).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookDispatchResult {
    pub additional_context: Vec<String>,
    pub decision: Option<HookDecision>,
    pub reason: Option<String>,
}

impl HookDispatchResult {
    /// Merge one handler's output into the aggregated result.
    ///
    /// Precedence: `Block` > `Ask` > `Allow`. The first `Block` reason wins.
    pub fn merge(&mut self, output: HookOutput) {
        if let Some(ctx) = output
            .additional_context
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            self.additional_context.push(ctx.to_string());
        }

        if let Some(next_decision) = output.decision {
            let replace = matches!(
                (self.decision, next_decision),
                (None, _)
                    | (
                        Some(HookDecision::Allow),
                        HookDecision::Ask | HookDecision::Block
                    )
                    | (Some(HookDecision::Ask), HookDecision::Block)
            );
            if replace {
                self.decision = Some(next_decision);
                if output.reason.is_some() {
                    self.reason = output.reason.clone();
                }
            }
        } else if self.reason.is_none() && output.reason.is_some() {
            self.reason = output.reason;
        }
    }

    pub fn joined_additional_context(&self) -> Option<String> {
        if self.additional_context.is_empty() {
            None
        } else {
            Some(self.additional_context.join("\n\n"))
        }
    }
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
) -> HookDispatchResult {
    let plugins_dir = match get_user_plugins_directory() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!(
                "plugin hook {} skipped for {}: plugins dir unavailable: {e}",
                event.as_str(),
                session_id
            );
            return HookDispatchResult::default();
        }
    };
    let session_plugins_dir = get_session_plugins_directory(session_id).ok();
    let session_trust_dir = get_session_plugin_trust_dir(session_id).ok();
    let mut resolved_params = envelope_params.clone();
    if resolved_params.cwd == Path::new(".") {
        if let Ok(cwd) = resolve_session_cwd(session_id, None).await {
            resolved_params.cwd = cwd;
        }
    }
    dispatch_hook_event_with_sources(
        &plugins_dir,
        session_plugins_dir.as_deref(),
        session_trust_dir.as_deref(),
        event,
        matcher_value,
        &resolved_params,
        session_id,
    )
    .await
}

pub async fn dispatch_hook_event_in(
    plugins_dir: &Path,
    event: HookEvent,
    matcher_value: &str,
    envelope_params: &HookEnvelopeParams,
    session_id: &str,
) -> HookDispatchResult {
    dispatch_hook_event_with_sources(
        plugins_dir,
        None,
        None,
        event,
        matcher_value,
        envelope_params,
        session_id,
    )
    .await
}

pub async fn dispatch_hook_event_with_sources(
    plugins_dir: &Path,
    session_plugins_dir: Option<&Path>,
    session_trust_dir: Option<&Path>,
    event: HookEvent,
    matcher_value: &str,
    envelope_params: &HookEnvelopeParams,
    session_id: &str,
) -> HookDispatchResult {
    let mut aggregated = HookDispatchResult::default();
    match event.dispatch_mode() {
        HookDispatchMode::Stub => {
            log::info!(
                "plugin hook {} not yet executed (stub) for session {}",
                event.as_str(),
                session_id
            );
            return aggregated;
        }
        HookDispatchMode::Execute => {}
    }

    let envelope = build_envelope_value(envelope_params);
    let envelope_json = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "{} hooks skipped for {}: envelope JSON encode failed: {e}",
                event.as_str(),
                session_id
            );
            return aggregated;
        }
    };

    // `CLAUDE_ENV_FILE` (#2926): only a SessionStart run gets one, and it is
    // truncated first so a stale file from an earlier run cannot leak values
    // into this session. Its `NAME=value` lines are applied to later shell
    // tool calls of the session once the hooks have run.
    let env_file = if event == HookEvent::SessionStart {
        prepare_session_env_file(session_id)
    } else {
        None
    };

    for record in
        list_effective_approved_sources_in(plugins_dir, session_plugins_dir, session_trust_dir)
    {
        let root = &record.root_dir;
        // Load exactly the files validated and approved at install, manifest
        // `hooks` entries included — not just the default discovery (G2.2).
        let config = match super::registry::hook_config_for_record(&record) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("skip plugin hooks under {}: {}", root.display(), e);
                continue;
            }
        };
        let Some(groups) = config.events.get(&event) else {
            continue;
        };
        // #2926: the variables this plugin's hooks run with. `userConfig`
        // values live in the option sidecar (plain values) and secure storage
        // (sensitive values); a secure-store read failure warns and leaves
        // that option absent rather than skipping the hook. A session-scope
        // sidecar is read from that session's trust directory, never from the
        // agent-writable workspace store (#2926 gatekeeper G1.1), and its
        // secure namespace is scoped to the session (G1.2).
        let session_scope =
            session_plugins_dir.is_some_and(|base| record.root_dir.starts_with(base));
        let (options, warning) = if session_scope {
            crate::services::skill_service::load_plugin_option_values(
                session_trust_dir.unwrap_or(session_plugins_dir.unwrap_or(plugins_dir)),
                &record.manifest.source_id,
                Some(session_id),
            )
            .await
        } else {
            crate::services::skill_service::load_plugin_option_values(
                plugins_dir,
                &record.manifest.source_id,
                None,
            )
            .await
        };
        if let Some(warning) = warning {
            log::warn!("{warning}");
        }
        let env = HookEnv {
            plugin_root: root.clone(),
            data_dir: plugin_data_dir_for(&record, session_id, session_plugins_dir),
            project_dir: envelope_params
                .cwd
                .to_str()
                .map(str::to_string)
                .unwrap_or_default(),
            env_file: env_file.clone(),
            options,
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
                if let Some(output) =
                    run_matching_handler(session_id, event, &env, handler, &envelope_json).await
                {
                    aggregated.merge(output);
                }
            }
        }
    }
    if let Some(path) = env_file {
        super::plugin_env::apply_env_file(session_id, &path);
    }
    aggregated
}

/// Create (or truncate) the session's `CLAUDE_ENV_FILE` and return it. A
/// failure only costs the plugin-variable feature, so it is logged, not fatal.
fn prepare_session_env_file(session_id: &str) -> Option<PathBuf> {
    let path = super::registry::get_session_plugin_env_file(session_id).ok()?;
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            log::warn!(
                "CLAUDE_ENV_FILE unavailable for session {}: create {}: {}",
                session_id,
                parent.display(),
                error
            );
            return None;
        }
    }
    match std::fs::write(&path, "") {
        Ok(()) => Some(path),
        Err(error) => {
            log::warn!(
                "CLAUDE_ENV_FILE unavailable for session {}: truncate {}: {}",
                session_id,
                path.display(),
                error
            );
            None
        }
    }
}

/// `${CLAUDE_PLUGIN_DATA}` of one source record (#2926): a session-store
/// record gets the session's data dir, anything else the global one. A path
/// that cannot be resolved falls back to the plugin root's sibling
/// `.plugin-data`, so a hook still gets a writable directory instead of none.
fn plugin_data_dir_for(
    record: &crate::services::hooks::registry::PluginSourceRecord,
    session_id: &str,
    session_plugins_dir: Option<&Path>,
) -> PathBuf {
    use super::registry::{get_session_plugin_data_dir, get_user_plugin_data_dir};
    let session_scope = session_plugins_dir.is_some_and(|base| record.root_dir.starts_with(base));
    let resolved = if session_scope {
        get_session_plugin_data_dir(session_id, &record.manifest.source_id)
    } else {
        get_user_plugin_data_dir(&record.manifest.source_id)
    };
    resolved.unwrap_or_else(|_| fallback_plugin_data_dir(record))
}

/// Data directory used when the data-dir root is unavailable (unit tests with
/// no session manager): keep the location stable per plugin source.
fn fallback_plugin_data_dir(
    record: &crate::services::hooks::registry::PluginSourceRecord,
) -> PathBuf {
    record
        .root_dir
        .parent()
        .map(|base| base.join(format!("{}-data", record.manifest.source_id)))
        .unwrap_or_else(|| record.root_dir.join(".plugin-data"))
}

async fn run_matching_handler(
    session_id: &str,
    event: HookEvent,
    env: &HookEnv,
    handler: &HookHandler,
    envelope_json: &str,
) -> Option<HookOutput> {
    match run_hook_handler(event, env, handler, envelope_json).await {
        HookRunOutcome::Applied(output) => {
            if event == HookEvent::SessionStart {
                if let Some(ctx) = output
                    .additional_context
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    append_hook_context(session_id, ctx);
                }
            }
            Some(output)
        }
        HookRunOutcome::Skipped(reason) => {
            log::debug!(
                "hook handler ({}) skipped for session {}: {}",
                event.as_str(),
                session_id,
                reason
            );
            None
        }
    }
}

enum HookRunOutcome {
    Applied(HookOutput),
    Skipped(String),
}

async fn run_hook_handler(
    event: HookEvent,
    env: &HookEnv,
    handler: &HookHandler,
    envelope_json: &str,
) -> HookRunOutcome {
    let timeout_secs = if handler.timeout_secs == 0 {
        DEFAULT_HOOK_TIMEOUT_SECS
    } else {
        handler.timeout_secs
    };
    let deadline = Duration::from_secs(timeout_secs);

    if let Some(http) = &handler.http {
        return run_http_hook(event, http, deadline, envelope_json).await;
    }

    let mut child = match build_hook_command(env, handler) {
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
    let parsed = normalize(&stdout, event);
    HookRunOutcome::Applied(parsed)
}

/// Environment a hook process runs with (#2926).
///
/// Before this, a hook saw only the plugin root aliases. The plugin variables
/// a Claude Code plugin can rely on are set here: `${CLAUDE_PLUGIN_DATA}`,
/// `CLAUDE_PROJECT_DIR`, `CLAUDE_ENV_FILE` (for `SessionStart` only), and one
/// `CLAUDE_PLUGIN_OPTION_<KEY>` per resolved `userConfig` value.
struct HookEnv {
    plugin_root: PathBuf,
    /// Per-plugin data directory, created before the hook runs.
    data_dir: PathBuf,
    /// The session's workspace directory (`CLAUDE_PROJECT_DIR`).
    project_dir: String,
    /// `CLAUDE_ENV_FILE`; only a `SessionStart` run gets one.
    env_file: Option<PathBuf>,
    /// Resolved `userConfig` values, keyed by the manifest key.
    options: std::collections::BTreeMap<String, String>,
}

impl HookEnv {
    /// Apply the plugin variables to a hook command. A data directory that
    /// cannot be created is logged and not fatal: the hook still runs, it just
    /// cannot write plugin data.
    fn apply(&self, cmd: &mut Command) {
        let root = self.plugin_root.to_string_lossy();
        cmd.env("LIBRAGENT_PLUGIN_ROOT", root.as_ref());
        cmd.env("CLAUDE_PLUGIN_ROOT", root.as_ref());
        cmd.env("CURSOR_PLUGIN_ROOT", root.as_ref());

        if let Err(error) = crate::services::hooks::registry::ensure_plugin_data_dir(&self.data_dir)
        {
            log::warn!("CLAUDE_PLUGIN_DATA is unusable for this hook: {error}");
        }
        let data = self.data_dir.to_string_lossy();
        cmd.env("LIBRAGENT_PLUGIN_DATA", data.as_ref());
        cmd.env("CLAUDE_PLUGIN_DATA", data.as_ref());

        cmd.env("LIBRAGENT_PROJECT_DIR", &self.project_dir);
        cmd.env("CLAUDE_PROJECT_DIR", &self.project_dir);

        if let Some(env_file) = self.env_file.as_deref() {
            let path = env_file.to_string_lossy();
            cmd.env("LIBRAGENT_ENV_FILE", path.as_ref());
            cmd.env("CLAUDE_ENV_FILE", path.as_ref());
        }

        for (key, value) in &self.options {
            if let Some(name) = crate::services::skill_service::option_env_name(key) {
                cmd.env(name, value);
            }
        }
    }
}

/// Interpolate `$VAR` / `${VAR}` references in HTTP hook header values
/// (#3009): names listed in `allowed_env_vars` resolve to their environment
/// value, unlisted names resolve to an empty string, and with no allowlist
/// nothing is interpolated at all.
fn interpolate_http_header(value: &str, allowed: &[String]) -> String {
    if allowed.is_empty() {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let brace = chars.peek() == Some(&'{');
        if brace {
            chars.next();
        }
        let mut name = String::new();
        while chars
            .peek()
            .is_some_and(|n| *n == '_' || n.is_ascii_alphanumeric())
        {
            name.push(chars.next().expect("peeked"));
        }
        let closed = !brace || chars.next() == Some('}');
        if name.is_empty() || !closed {
            out.push('$');
            if brace {
                out.push('{');
            }
            continue;
        }
        let resolved = if allowed.iter().any(|v| v == &name) {
            std::env::var(&name).unwrap_or_default()
        } else {
            String::new()
        };
        out.push_str(&resolved);
    }
    out
}

/// Run a `type: "http"` hook: POST the event envelope as JSON to `url` and
/// apply the response body per the Hooks reference HTTP response handling
/// (2xx + empty body = success with no output, 2xx + JSON object = the same
/// JSON output schema as command hooks, anything else = non-blocking error)
/// (#3009).
async fn run_http_hook(
    event: HookEvent,
    http: &HookHttpConfig,
    deadline: Duration,
    envelope_json: &str,
) -> HookRunOutcome {
    use std::str::FromStr;

    let client = reqwest::Client::new();
    let mut request = client
        .post(&http.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    for (name, value) in &http.headers {
        match reqwest::header::HeaderName::from_str(name.as_str()) {
            Ok(header_name) => {
                let resolved = interpolate_http_header(value, &http.allowed_env_vars);
                let header_value = reqwest::header::HeaderValue::from_str(&resolved)
                    .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static(""));
                request = request.header(header_name, header_value);
            }
            Err(error) => {
                log::warn!("http hook: invalid header name {name:?}: {error}");
            }
        }
    }
    let request = request.body(envelope_json.to_string());

    let response = match timeout(deadline, request.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            return HookRunOutcome::Skipped(format!("http hook request failed: {error}"));
        }
        Err(_) => {
            return HookRunOutcome::Skipped(format!(
                "http hook timed out after {}s",
                deadline.as_secs()
            ));
        }
    };
    let status = response.status();
    if !status.is_success() {
        return HookRunOutcome::Skipped(format!("http hook status {}", status.as_u16()));
    }
    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            return HookRunOutcome::Skipped(format!("http hook body read failed: {error}"));
        }
    };
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return HookRunOutcome::Applied(HookOutput::default());
    }
    // The body must be a JSON object using the command-hook output schema;
    // any other body (e.g. plain text) is a non-blocking error.
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return HookRunOutcome::Skipped("http hook body is not JSON".to_string());
    };
    if !value.is_object() {
        return HookRunOutcome::Skipped("http hook body is not a JSON object".to_string());
    }
    HookRunOutcome::Applied(normalize(trimmed, event))
}

fn build_hook_command(env: &HookEnv, handler: &HookHandler) -> Result<Command, String> {
    let mut cmd = build_hook_shell_command(&env.plugin_root, handler)?;
    env.apply(&mut cmd);
    Ok(cmd)
}

fn build_hook_shell_command(plugin_root: &Path, handler: &HookHandler) -> Result<Command, String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(ps) = handler.powershell.as_deref() {
            let script = expand_plugin_root_placeholders(ps, plugin_root);
            let mut cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-Command", &script]);
            cmd.current_dir(plugin_root);
            return Ok(cmd);
        }
        let script = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/C", &script]);
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
        compute_handler_approval_hashes, record_session_plugin_trust, save_source_in,
        upsert_session_store_entry, PluginSourceManifest, SessionPluginStoreEntry,
        SessionPluginTrust,
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
                enabled: true,
                installed_commit: None,
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
                enabled: true,
                installed_commit: None,
            },
        )
        .unwrap();
        (temp, root)
    }

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

    /// Serves exactly one HTTP request, returning `response_body` after
    /// checking the request line and headers contain `assertions` substrings.
    /// Returns the accepted request body.
    async fn serve_one_http_request(
        listener: tokio::net::TcpListener,
        response_body: &'static str,
        assertions: Vec<&'static str>,
    ) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut conn, _) = listener.accept().await.expect("accept");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end;
        loop {
            let n = conn.read(&mut chunk).await.expect("read request");
            if n == 0 {
                panic!("request ended before headers were complete");
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
        let content_length: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        while buf.len() < header_end + content_length {
            let n = conn.read(&mut chunk).await.expect("read body");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
        let haystack = format!(
            "{}\n{body}",
            String::from_utf8_lossy(&buf[..header_end]).to_lowercase()
        );
        for needle in assertions {
            assert!(
                haystack.contains(needle),
                "request should contain {needle:?}, got: {haystack}"
            );
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        conn.write_all(response.as_bytes())
            .await
            .expect("write response");
        body
    }

    /// #3009: a `type: "http"` handler POSTs the event envelope and the
    /// 2xx JSON body is applied with the same decision fields as command
    /// hooks. Also checks header interpolation follows `allowedEnvVars`.
    #[tokio::test]
    async fn http_handler_posts_envelope_and_applies_json_decision() {
        std::env::set_var("TEST_HOOK_TOKEN", "tok-42");
        let json_out: &'static str = r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"http says no"}}"#;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let url = format!("http://{}/pre-tool-use", listener.local_addr().unwrap());
        let server = tokio::spawn(serve_one_http_request(
            listener,
            json_out,
            vec![
                "content-type: application/json",
                "authorization: bearer tok-42",
                "x-unlisted: ",
                "hook_event_name",
            ],
        ));

        let hooks_json = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "workspace__runShell",
                    "hooks": [{
                        "type": "http",
                        "url": url,
                        "headers": {
                            "Authorization": "Bearer $TEST_HOOK_TOKEN",
                            "X-Unlisted": "[$NOT_LISTED]"
                        },
                        "allowedEnvVars": ["TEST_HOOK_TOKEN"]
                    }]
                }]
            }
        })
        .to_string();
        let (temp, root) = setup_approved_plugin(&hooks_json);

        let sid = format!("http-hook-{}", uuid::Uuid::new_v4());
        let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
        params.tool_name = Some("workspace__runShell".to_string());
        let outcome = dispatch_hook_event_in(
            temp.path(),
            HookEvent::PreToolUse,
            "workspace__runShell",
            &params,
            &sid,
        )
        .await;
        assert_eq!(
            outcome.decision,
            Some(HookDecision::Block),
            "http response decision fields must apply"
        );
        assert_eq!(outcome.reason.as_deref(), Some("http says no"));

        let body = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .expect("server task")
            .expect("server task join");
        assert!(
            body.contains("workspace__runShell"),
            "POST body is the event envelope: {body}"
        );
        std::env::remove_var("TEST_HOOK_TOKEN");
        clear_hook_context(&sid);
    }

    /// #3009: a non-2xx response is a non-blocking error; the action goes
    /// ahead with no decision from the hook.
    #[tokio::test]
    async fn http_handler_non_2xx_is_non_blocking() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut conn, _) = listener.accept().await.expect("accept");
            let mut buf = [0u8; 1024];
            let _ = conn.read(&mut buf).await;
            let response = "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            conn.write_all(response.as_bytes()).await.expect("write");
        });

        let hooks_json = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "workspace__runShell",
                    "hooks": [{ "type": "http", "url": format!("http://{addr}/fail") }]
                }]
            }
        })
        .to_string();
        let (temp, root) = setup_approved_plugin(&hooks_json);

        let sid = format!("http-hook-500-{}", uuid::Uuid::new_v4());
        let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
        let outcome = dispatch_hook_event_in(
            temp.path(),
            HookEvent::PreToolUse,
            "workspace__runShell",
            &params,
            &sid,
        )
        .await;
        assert_eq!(outcome.decision, None, "non-2xx must not block");

        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
        clear_hook_context(&sid);
    }

    #[test]
    fn http_header_interpolation_follows_allowlist() {
        std::env::set_var("TEST_INTERP_A", "value-a");
        let allowed: Vec<String> = vec![
            "TEST_INTERP_A".to_string(),
            "TEST_INTERP_MISSING".to_string(),
        ];
        assert_eq!(
            interpolate_http_header("Bearer ${TEST_INTERP_A} and $TEST_INTERP_A", &allowed),
            "Bearer value-a and value-a"
        );
        assert_eq!(
            interpolate_http_header("[$TEST_INTERP_MISSING]", &allowed),
            "[]",
            "allowlisted but unset resolves to empty"
        );
        assert_eq!(
            interpolate_http_header("[$NOT_ALLOWED]", &allowed),
            "[]",
            "unlisted references become empty"
        );
        assert_eq!(
            interpolate_http_header("$TEST_INTERP_A", &[]),
            "$TEST_INTERP_A",
            "no allowlist means no interpolation"
        );
        std::env::remove_var("TEST_INTERP_A");
    }

    #[tokio::test]
    async fn user_prompt_submit_returns_additional_context_and_block() {
        let json_out = r#"{"additionalContext":"turn-only extra context"}"#;
        let ps = format!("Write-Output '{json_out}'");
        let sh = format!("printf '%s' '{json_out}'");
        let hooks_json = serde_json::json!({
            "hooks": {
                "UserPromptSubmit": [{
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

        let sid = format!("prompt-submit-{}", uuid::Uuid::new_v4());
        let mut params = HookEnvelopeParams::new(&sid, root, HookEvent::UserPromptSubmit);
        params.prompt = Some("hello".to_string());
        let outcome =
            dispatch_hook_event_in(temp.path(), HookEvent::UserPromptSubmit, "", &params, &sid)
                .await;
        assert_eq!(
            outcome.joined_additional_context().as_deref(),
            Some("turn-only extra context")
        );
        assert_eq!(outcome.decision, None);
        // Must NOT pollute persistent SessionStart context store.
        assert!(get_hook_context(&sid).is_none());
        clear_hook_context(&sid);
    }

    #[tokio::test]
    async fn post_tool_use_and_stop_and_session_end_and_notification_and_pre_compact_execute() {
        let marker_dir = tempfile::tempdir().expect("marker dir");
        let end_marker = marker_dir.path().join("session-end.txt");
        let notif_marker = marker_dir.path().join("notif.txt");
        let compact_marker = marker_dir.path().join("compact.txt");

        let end_str = end_marker.to_string_lossy().to_string();
        let notif_str = notif_marker.to_string_lossy().to_string();
        let compact_str = compact_marker.to_string_lossy().to_string();

        let stop_json = r#"{"decision":"block","reason":"verify tests before stopping"}"#;
        let post_json = r#"{"additionalContext":"post-tool hint"}"#;

        let hooks_json = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "command": format!("printf '%s' '{stop_json}'"),
                    "powershell": format!("Write-Output '{stop_json}'"),
                }],
                "PostToolUse": [{
                    "matcher": "workspace__writeFile",
                    "hooks": [{
                        "type": "command",
                        "command": format!("printf '%s' '{post_json}'"),
                        "powershell": format!("Write-Output '{post_json}'"),
                    }]
                }],
                "SessionEnd": [{
                    "command": format!("sh -c 'echo end > \"{end_str}\"'"),
                    "powershell": format!("Set-Content -Path '{end_str}' -Value 'end'"),
                }],
                "Notification": [{
                    "matcher": "permission",
                    "hooks": [{
                        "type": "command",
                        "command": format!("sh -c 'echo notif > \"{notif_str}\"'"),
                        "powershell": format!("Set-Content -Path '{notif_str}' -Value 'notif'"),
                    }]
                }],
                "PreCompact": [{
                    "matcher": "auto",
                    "hooks": [{
                        "type": "command",
                        "command": format!("sh -c 'echo compact > \"{compact_str}\"'"),
                        "powershell": format!("Set-Content -Path '{compact_str}' -Value 'compact'"),
                    }]
                }]
            }
        })
        .to_string();
        let (temp, root) = setup_approved_plugin(&hooks_json);
        let sid = format!("multi-event-{}", uuid::Uuid::new_v4());

        let stop_res = dispatch_hook_event_in(
            temp.path(),
            HookEvent::Stop,
            "",
            &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::Stop),
            &sid,
        )
        .await;
        assert_eq!(stop_res.decision, Some(HookDecision::Block));
        assert_eq!(
            stop_res.reason.as_deref(),
            Some("verify tests before stopping")
        );

        let post_res = dispatch_hook_event_in(
            temp.path(),
            HookEvent::PostToolUse,
            "workspace__writeFile",
            &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PostToolUse),
            &sid,
        )
        .await;
        assert_eq!(
            post_res.joined_additional_context().as_deref(),
            Some("post-tool hint")
        );

        let _ = dispatch_hook_event_in(
            temp.path(),
            HookEvent::SessionEnd,
            "delete",
            &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::SessionEnd),
            &sid,
        )
        .await;
        assert!(end_marker.is_file(), "SessionEnd handler should execute");

        let _ = dispatch_hook_event_in(
            temp.path(),
            HookEvent::Notification,
            "permission",
            &HookEnvelopeParams::new(&sid, root.clone(), HookEvent::Notification),
            &sid,
        )
        .await;
        assert!(
            notif_marker.is_file(),
            "Notification handler should execute"
        );

        let _ = dispatch_hook_event_in(
            temp.path(),
            HookEvent::PreCompact,
            "auto",
            &HookEnvelopeParams::new(&sid, root, HookEvent::PreCompact),
            &sid,
        )
        .await;
        assert!(
            compact_marker.is_file(),
            "PreCompact handler should execute"
        );
        clear_hook_context(&sid);
    }

    #[test]
    fn decision_aggregation_precedence_is_block_over_ask_over_allow() {
        let mut res = HookDispatchResult::default();
        res.merge(HookOutput {
            decision: Some(HookDecision::Allow),
            reason: Some("allowed".to_string()),
            ..HookOutput::default()
        });
        assert_eq!(res.decision, Some(HookDecision::Allow));

        res.merge(HookOutput {
            decision: Some(HookDecision::Ask),
            reason: Some("confirm".to_string()),
            ..HookOutput::default()
        });
        assert_eq!(res.decision, Some(HookDecision::Ask));
        assert_eq!(res.reason.as_deref(), Some("confirm"));

        res.merge(HookOutput {
            decision: Some(HookDecision::Block),
            reason: Some("denied".to_string()),
            ..HookOutput::default()
        });
        assert_eq!(res.decision, Some(HookDecision::Block));
        assert_eq!(res.reason.as_deref(), Some("denied"));

        // Subsequent Allow/Ask cannot downgrade Block.
        res.merge(HookOutput {
            decision: Some(HookDecision::Allow),
            reason: Some("too late".to_string()),
            ..HookOutput::default()
        });
        assert_eq!(res.decision, Some(HookDecision::Block));
        assert_eq!(res.reason.as_deref(), Some("denied"));
    }
    // --- #2825 session-scoped plugin sources ---------------------------------

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
}

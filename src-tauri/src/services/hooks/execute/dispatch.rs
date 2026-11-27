//! Public dispatch entrypoints: session start, stub events, and hook event
//! dispatch with its result aggregation (#2825).

use super::command::HookEnv;
use super::env::{plugin_data_dir_for, prepare_cwd_env_file, prepare_session_env_file};
use super::handler::run_matching_handler;
use super::matcher::{
    if_condition_matches, matcher_matches, resolve_session_cwd, session_start_source_label,
};
use crate::services::hooks::context::{
    begin_hook_replacement, begin_hook_run, end_hook_run, finish_hook_replacement,
};
use crate::services::hooks::envelope::{
    build_envelope_value, HookEnvelopeParams, SessionStartSource,
};
use crate::services::hooks::output::{HookDecision, HookOutput};
use crate::services::hooks::registry::list_effective_approved_sources_in;
use crate::services::hooks::registry::{
    approval_hash_key, approval_hash_value, get_session_plugin_trust_dir,
    get_session_plugins_directory, get_user_plugins_directory,
};
use crate::services::hooks::schema::{HookDispatchMode, HookEvent};
use std::path::{Path, PathBuf};

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
    crate::services::hooks::lifecycle::note_session_start(session_id, source);
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
    /// `updatedInput` from the handler that last allowed the call (#3005).
    /// `None` means no hook rewrote the arguments.
    pub updated_input: Option<serde_json::Value>,
    /// Aggregated `continue: false` from any handler (#3004). Once a hook stops
    /// processing, a later `continue: true` cannot undo it.
    pub stop_reason: Option<String>,
    pub stops_processing: bool,
    /// `displayContent` from the last `MessageDisplay` handler that set one.
    pub display_content: Option<String>,
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

        if output.display_content.is_some() {
            self.display_content = output.display_content;
        }

        // Claude Code applies one rewrite per call, so the last allowed
        // handler's input replaces any earlier one.
        if output.updated_input.is_some() && output.decision != Some(HookDecision::Block) {
            self.updated_input = output.updated_input;
        }

        // `continue: false` (#3004): the first handler that stops wins, and its
        // `stopReason` is the text the user sees. A later `continue: true` is a
        // no-op, so one plugin cannot re-enable processing another stopped.
        if output.continue_loop == Some(false) && !self.stops_processing {
            self.stops_processing = true;
            self.stop_reason = output.stop_reason;
        }
    }

    /// The `stopReason` of a hook that stopped processing, or `fallback`.
    pub fn stop_message(&self, fallback: &str) -> String {
        self.stop_reason
            .clone()
            .unwrap_or_else(|| fallback.to_string())
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

/// Injectable hook dispatch seam so production runs installed plugin hooks
/// while tests can record emissions without touching the user profile.
#[async_trait::async_trait]
pub trait HookDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        event: HookEvent,
        matcher_value: &str,
        envelope_params: &HookEnvelopeParams,
        session_id: &str,
    ) -> HookDispatchResult;

    fn spawn_session_start(&self, session_id: &str, source: SessionStartSource);
}

pub struct DefaultHookDispatcher;

#[async_trait::async_trait]
impl HookDispatcher for DefaultHookDispatcher {
    async fn dispatch(
        &self,
        event: HookEvent,
        matcher_value: &str,
        envelope_params: &HookEnvelopeParams,
        session_id: &str,
    ) -> HookDispatchResult {
        dispatch_hook_event(event, matcher_value, envelope_params, session_id).await
    }

    fn spawn_session_start(&self, session_id: &str, source: SessionStartSource) {
        spawn_session_start(session_id, source);
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObservedHookDispatch {
    pub event: HookEvent,
    pub matcher_value: String,
    pub session_id: String,
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingHookDispatcher {
    events: std::sync::Mutex<Vec<ObservedHookDispatch>>,
    notify: tokio::sync::Notify,
}

#[cfg(test)]
impl RecordingHookDispatcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn events_for_session(&self, session_id: &str) -> Vec<ObservedHookDispatch> {
        self.events
            .lock()
            .expect("recorded hook events lock")
            .iter()
            .filter(|entry| entry.session_id == session_id)
            .cloned()
            .collect()
    }

    pub(crate) async fn wait_for(
        &self,
        session_id: &str,
        event: HookEvent,
        timeout: std::time::Duration,
    ) -> Option<ObservedHookDispatch> {
        tokio::time::timeout(timeout, async {
            loop {
                let notified = self.notify.notified();
                if let Some(found) = self
                    .events_for_session(session_id)
                    .into_iter()
                    .find(|entry| entry.event == event)
                {
                    return found;
                }
                notified.await;
            }
        })
        .await
        .ok()
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl HookDispatcher for RecordingHookDispatcher {
    async fn dispatch(
        &self,
        event: HookEvent,
        matcher_value: &str,
        _envelope_params: &HookEnvelopeParams,
        session_id: &str,
    ) -> HookDispatchResult {
        self.events
            .lock()
            .expect("recorded hook events lock")
            .push(ObservedHookDispatch {
                event,
                matcher_value: matcher_value.to_string(),
                session_id: session_id.to_string(),
            });
        self.notify.notify_waiters();
        HookDispatchResult::default()
    }

    fn spawn_session_start(&self, session_id: &str, source: SessionStartSource) {
        self.events
            .lock()
            .expect("recorded hook events lock")
            .push(ObservedHookDispatch {
                event: HookEvent::SessionStart,
                matcher_value: session_start_source_label(source).to_string(),
                session_id: session_id.to_string(),
            });
        self.notify.notify_waiters();
    }
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

    // `CLAUDE_ENV_FILE` (#2926): `SessionStart` and `CwdChanged` runs get one,
    // each in its own file. It is truncated first so a stale file from an
    // earlier run cannot leak values into this session. Its `NAME=value` lines
    // are applied to later shell tool calls of the session once the hooks have
    // run. Claude Code also sets it for `Setup` and `FileChanged`, which
    // LibrAgent does not emit.
    let env_file = match event {
        HookEvent::SessionStart => prepare_session_env_file(session_id),
        HookEvent::CwdChanged => prepare_cwd_env_file(session_id),
        _ => None,
    };

    for record in
        list_effective_approved_sources_in(plugins_dir, session_plugins_dir, session_trust_dir)
    {
        let root = &record.root_dir;
        // Load exactly the files validated and approved at install, manifest
        // `hooks` entries included — not just the default discovery (G2.2).
        let config = match crate::services::hooks::registry::hook_config_for_record(&record) {
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
            if event.supports_matcher() && !matcher_matches(group.matcher.as_deref(), matcher_value)
            {
                continue;
            }
            for handler in &group.handlers {
                if !if_condition_matches(
                    handler.r#if.as_deref(),
                    event,
                    envelope_params.tool_name.as_deref(),
                    envelope_params.tool_input.as_ref(),
                ) {
                    continue;
                }
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
        use crate::services::hooks::plugin_env::{apply_cwd_env_file, apply_env_file};
        if event == HookEvent::CwdChanged {
            apply_cwd_env_file(session_id, &path);
        } else {
            apply_env_file(session_id, &path);
        }
    }
    aggregated
}

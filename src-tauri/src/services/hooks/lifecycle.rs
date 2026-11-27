//! LibrAgent lifecycle points for the prompt, message, instruction, setting,
//! directory, and model hook events (#3008). Each function builds the Claude
//! Code stdin fields of one event, dispatches it, and applies the part of the
//! output that event accepts.
//! <https://code.claude.com/docs/en/hooks>

use crate::services::hooks::context::push_turn_hook_context;
use crate::services::hooks::envelope::{HookEnvelopeParams, SessionStartSource};
use crate::services::hooks::execute::{
    dispatch_hook_event_with_sources, resolve_session_cwd, HookDispatchResult,
};
use crate::services::hooks::output::HookDecision;
use crate::services::hooks::registry::{
    get_session_plugin_trust_dir, get_session_plugins_directory, get_user_plugins_directory,
};
use crate::services::hooks::schema::HookEvent;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The plugin stores one dispatch reads.
#[derive(Debug, Clone)]
pub struct HookSources {
    pub plugins_dir: PathBuf,
    pub session_plugins_dir: Option<PathBuf>,
    pub session_trust_dir: Option<PathBuf>,
}

impl HookSources {
    /// Global plugins only, for events that belong to no session.
    pub fn global() -> Option<Self> {
        match get_user_plugins_directory() {
            Ok(plugins_dir) => Some(Self {
                plugins_dir,
                session_plugins_dir: None,
                session_trust_dir: None,
            }),
            Err(error) => {
                log::warn!("plugin hooks skipped: plugins dir unavailable: {error}");
                None
            }
        }
    }

    /// Global plugins plus the plugins installed for `session_id`.
    pub fn for_session(session_id: &str) -> Option<Self> {
        let mut sources = Self::global()?;
        sources.session_plugins_dir = get_session_plugins_directory(session_id).ok();
        sources.session_trust_dir = get_session_plugin_trust_dir(session_id).ok();
        Some(sources)
    }
}

async fn emit(
    sources: &HookSources,
    session_id: &str,
    event: HookEvent,
    matcher_value: &str,
    fields: Value,
) -> HookDispatchResult {
    // An app-wide event has no session and therefore no workspace.
    let cwd = if session_id.is_empty() {
        sources.plugins_dir.clone()
    } else {
        resolve_session_cwd(session_id, None)
            .await
            .unwrap_or_else(|_| sources.plugins_dir.clone())
    };
    let mut params = HookEnvelopeParams::new(session_id, cwd, event);
    params.extra = match fields {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    dispatch_hook_event_with_sources(
        &sources.plugins_dir,
        sources.session_plugins_dir.as_deref(),
        sources.session_trust_dir.as_deref(),
        event,
        matcher_value,
        &params,
        session_id,
    )
    .await
}

fn is_blocked(result: &HookDispatchResult) -> bool {
    result.decision == Some(HookDecision::Block) || result.stops_processing
}

/// `UserPromptExpansion`: a typed plugin command is about to expand into a
/// prompt. `Err` blocks the expansion with the reason shown to the user; added
/// context travels with the expanded prompt's model request.
pub async fn user_prompt_expansion(
    sources: &HookSources,
    session_id: &str,
    command_name: &str,
    command_args: &str,
    prompt: &str,
) -> Result<(), String> {
    let result = emit(
        sources,
        session_id,
        HookEvent::UserPromptExpansion,
        command_name,
        json!({
            "expansion_type": "slash_command",
            "command_name": command_name,
            "command_args": command_args,
            "command_source": "plugin",
            "prompt": prompt,
        }),
    )
    .await;
    if is_blocked(&result) {
        return Err(result
            .stop_reason
            .or(result.reason)
            .unwrap_or_else(|| "Command blocked by a UserPromptExpansion hook".to_string()));
    }
    if let Some(context) = result.joined_additional_context() {
        push_turn_hook_context(session_id, &context);
    }
    Ok(())
}

/// `MessageDisplay`: an assistant message is about to be shown. Returns the
/// text to show instead, if a hook set one. LibrAgent calls this once per
/// completed message, so the batch is always `index: 0`, `final: true`.
pub async fn message_display(
    sources: &HookSources,
    session_id: &str,
    turn_id: &str,
    message_id: &str,
    text: &str,
) -> Option<String> {
    emit(
        sources,
        session_id,
        HookEvent::MessageDisplay,
        "",
        json!({
            "turn_id": turn_id,
            "message_id": message_id,
            "index": 0,
            "final": true,
            "delta": text,
        }),
    )
    .await
    .display_content
}

#[derive(Default)]
struct InstructionState {
    announced: HashSet<PathBuf>,
    after_compact: bool,
    reload_pending: bool,
}

fn instruction_states() -> &'static Mutex<HashMap<String, InstructionState>> {
    static STATES: once_cell::sync::Lazy<Mutex<HashMap<String, InstructionState>>> =
        once_cell::sync::Lazy::new(Default::default);
    &STATES
}

/// Forget which instruction files a session has announced when its context is
/// rebuilt: compaction reloads them with `load_reason: "compact"`, startup and
/// `/clear` with `"session_start"`.
pub fn note_session_start(session_id: &str, source: SessionStartSource) {
    if source == SessionStartSource::Resume {
        return;
    }
    if let Ok(mut states) = instruction_states().lock() {
        states.insert(
            session_id.to_string(),
            InstructionState {
                announced: HashSet::new(),
                after_compact: source == SessionStartSource::Compact,
                reload_pending: source != SessionStartSource::Startup,
            },
        );
    }
}

/// Whether the session's instructions must be announced again even though the
/// cached prompt prefix is reused. Consumed by the call.
pub fn take_instruction_reload(session_id: &str) -> bool {
    instruction_states()
        .lock()
        .ok()
        .and_then(|mut states| {
            states
                .get_mut(session_id)
                .map(|state| std::mem::take(&mut state.reload_pending))
        })
        .unwrap_or(false)
}

pub fn clear_instruction_state(session_id: &str) {
    if let Ok(mut states) = instruction_states().lock() {
        states.remove(session_id);
    }
}

/// The `load_reason` to report for `path`, or `None` when this context already
/// announced it.
fn claim_instruction_load(session_id: &str, path: &Path) -> Option<&'static str> {
    let mut states = instruction_states().lock().ok()?;
    let state = states.entry(session_id.to_string()).or_default();
    if !state.announced.insert(path.to_path_buf()) {
        return None;
    }
    Some(if state.after_compact {
        "compact"
    } else {
        "session_start"
    })
}

/// `InstructionsLoaded`: a workspace instruction file entered the context.
/// Observational: the output is ignored.
pub async fn instructions_loaded(sources: &HookSources, session_id: &str, path: &Path) {
    let Some(load_reason) = claim_instruction_load(session_id, path) else {
        return;
    };
    emit(
        sources,
        session_id,
        HookEvent::InstructionsLoaded,
        load_reason,
        json!({
            "file_path": path.to_string_lossy(),
            "memory_type": "Project",
            "load_reason": load_reason,
        }),
    )
    .await;
}

/// `ConfigChange` for the app settings (`source: "user_settings"`). `Err` means
/// a hook blocked the change; the hook's reason is only logged.
pub async fn config_change(sources: &HookSources) -> Result<(), String> {
    let result = emit(
        sources,
        "",
        HookEvent::ConfigChange,
        "user_settings",
        json!({ "source": "user_settings" }),
    )
    .await;
    // `continue` is ignored for this event; only a block decision counts.
    if result.decision == Some(HookDecision::Block) {
        if let Some(reason) = result.reason {
            log::debug!("ConfigChange hook blocked a settings change: {reason}");
        }
        return Err("Settings change blocked by a ConfigChange hook".to_string());
    }
    Ok(())
}

/// `CwdChanged`: a shell command moved the session's shell. Observational, but
/// the hooks may write `CLAUDE_ENV_FILE`.
pub async fn cwd_changed(sources: &HookSources, session_id: &str, old_cwd: &str, new_cwd: &str) {
    emit(
        sources,
        session_id,
        HookEvent::CwdChanged,
        "",
        json!({ "old_cwd": old_cwd, "new_cwd": new_cwd }),
    )
    .await;
}

/// One model switch of one session.
#[derive(Debug, Clone)]
pub struct ModelSwitch {
    pub from_model: String,
    pub to_model: String,
    /// `"picker"` for the desktop model picker, `"sdk"` for a programmatic
    /// request.
    pub source: &'static str,
    /// Latest provider-reported prompt tokens, when known.
    pub context_tokens: Option<i64>,
}

impl ModelSwitch {
    // `prompt_cache_warm`, `cache_ttl`, `estimated_cache_write_usd`, and
    // `pricing` are omitted: LibrAgent does not track them.
    fn fields(&self) -> Value {
        let mut fields = json!({
            "from_model": self.from_model,
            "to_model": self.to_model,
            "requested_model": self.to_model,
            "source": self.source,
        });
        if let Some(tokens) = self.context_tokens {
            fields["context_tokens"] = json!(tokens);
        }
        fields
    }
}

/// `PreModelSwitch`: `Err` cancels the switch. `ask` also cancels it, because
/// no LibrAgent switch path can prompt for confirmation.
pub async fn pre_model_switch(
    sources: &HookSources,
    session_id: &str,
    switch: &ModelSwitch,
) -> Result<(), String> {
    let result = emit(
        sources,
        session_id,
        HookEvent::PreModelSwitch,
        &switch.to_model,
        switch.fields(),
    )
    .await;
    if is_blocked(&result) || result.decision == Some(HookDecision::Ask) {
        return Err(format!(
            "Model switch blocked by a PreModelSwitch hook{}",
            result
                .reason
                .map(|reason| format!(": {reason}"))
                .unwrap_or_default()
        ));
    }
    Ok(())
}

/// `PostModelSwitch`: the switch happened. Added context reaches the next model
/// request.
pub async fn post_model_switch(sources: &HookSources, session_id: &str, switch: &ModelSwitch) {
    let result = emit(
        sources,
        session_id,
        HookEvent::PostModelSwitch,
        &switch.to_model,
        switch.fields(),
    )
    .await;
    if let Some(context) = result.joined_additional_context() {
        push_turn_hook_context(session_id, &context);
    }
}

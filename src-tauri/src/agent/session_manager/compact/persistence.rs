use super::context::save_compact_context;
use super::CompactResponseOutcome;
use crate::agent::compaction_telemetry::{CompactionTelemetryEventDraft, CompactionTelemetryPhase};
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::CompactStatePhase;
use crate::agent::state::{AgentSession, CompactionResumeAction};
use crate::agent::tauri_events::{emit_compact_request, TauriEventDispatcher};
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, MessageRepository, SessionRepository};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_SUMMARY_RETRY_ATTEMPTS: u32 = 3;

pub(super) struct CompactSummaryPersistenceContext<'a> {
    /// The fields the testable path needs; declared once, in the data type.
    pub data: CompactSummaryPersistenceData<'a>,
    /// Production-only handles used to build the Tauri runtime.
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
}

/// Injectable runtime effects for compaction persistence so production uses
/// Tauri `AppHandle` while tests can drive the real path without optional skip branches.
#[async_trait::async_trait]
pub(super) trait CompactPersistenceRuntime: Send + Sync {
    fn emit_compact_finished(
        &self,
        session_id: String,
        session_name: Option<String>,
        phase: CompactStatePhase,
        error: Option<String>,
    ) -> Result<(), String>;
    fn spawn_resume_completion(&self, session_id: &str, log_label: &'static str);
    async fn start_workflow_from_pending_queue(&self, session_id: &str) -> Result<(), String>;
}

struct TauriCompactPersistenceRuntime<'a> {
    session_repo: &'a Arc<dyn SessionRepository>,
    active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &'a Arc<MCPServiceProxyManager>,
    app_handle: &'a AppHandle,
}

#[async_trait::async_trait]
impl CompactPersistenceRuntime for TauriCompactPersistenceRuntime<'_> {
    fn emit_compact_finished(
        &self,
        session_id: String,
        session_name: Option<String>,
        phase: CompactStatePhase,
        error: Option<String>,
    ) -> Result<(), String> {
        let dispatcher = TauriEventDispatcher::from_handle(self.app_handle);
        dispatcher.emit_compact_finished(session_id, session_name, phase, error)
    }

    fn spawn_resume_completion(&self, session_id: &str, log_label: &'static str) {
        spawn_resume_completion(
            self.session_repo,
            self.active_sessions,
            self.proxy_manager,
            self.app_handle,
            session_id,
            log_label,
        );
    }

    async fn start_workflow_from_pending_queue(&self, session_id: &str) -> Result<(), String> {
        crate::agent::workflow::compact_pending::start_workflow_from_pending_queue(
            self.session_repo,
            self.active_sessions,
            self.proxy_manager,
            self.app_handle,
            session_id,
        )
        .await
    }
}

pub(super) struct CompactSummaryPersistenceData<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub session_id: &'a str,
    pub session_name: Option<String>,
    pub to_id: String,
    pub compacted_delta_count: usize,
    pub telemetry_phase: CompactionTelemetryPhase,
    pub epoch_path: Option<String>,
    pub fallback_path: Option<String>,
    pub raw_output_path: Option<String>,
}

pub(super) fn spawn_resume_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    log_label: &'static str,
) {
    let session_repo = session_repo.clone();
    let active_sessions = active_sessions.clone();
    let proxy_manager = proxy_manager.clone();
    let app_handle = app_handle.clone();
    let resume_session_id = session_id.to_string();

    tokio::spawn(async move {
        if let Err(error) = crate::agent::llm::request_llm_completion_with_recovery(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            resume_session_id.clone(),
        )
        .await
        {
            log::error!(
                "Failed to resume {} after compaction for session {}: {}",
                log_label,
                resume_session_id,
                error
            );
        }
    });
}

fn clear_message_prompt_token_checkpoint(message: &mut Message) -> bool {
    let mut changed = false;

    if message.prompt_tokens.take().is_some() {
        changed = true;
    }

    if let Some(usage) = message
        .usage
        .as_mut()
        .and_then(|usage| usage.as_object_mut())
    {
        if usage.remove("promptTokens").is_some() {
            changed = true;
        }
    }

    changed
}

pub fn clear_message_prompt_token_checkpoint_for_testing(message: &mut Message) -> bool {
    clear_message_prompt_token_checkpoint(message)
}

async fn invalidate_retained_tail_prompt_token_checkpoints(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    to_id: &str,
) -> Result<(), String> {
    let changed_messages = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return Ok(());
        };

        let messages = session.messages.read().await;
        let Some(to_idx) = messages.iter().position(|message| message.id == to_id) else {
            return Ok(());
        };

        messages
            .iter()
            .skip(to_idx.saturating_add(1))
            .filter_map(|message| {
                let mut updated_message = message.clone();
                if clear_message_prompt_token_checkpoint(&mut updated_message) {
                    Some(updated_message)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    if changed_messages.is_empty() {
        return Ok(());
    }

    crate::state::get_message_repository()
        .insert_many(changed_messages.clone())
        .await
        .map_err(|error| error.to_string())?;

    let changed_by_id = changed_messages
        .into_iter()
        .map(|message| (message.id.clone(), message))
        .collect::<HashMap<_, _>>();

    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Ok(());
    };
    let mut messages = session.messages.write().await;
    for message in messages.iter_mut() {
        if let Some(updated_message) = changed_by_id.get(&message.id) {
            *message = updated_message.clone();
        }
    }

    Ok(())
}

pub(super) async fn retry_invalid_compact_summary_if_possible(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    validation_error: &str,
) -> Result<Option<CompactResponseOutcome>, String> {
    let compaction = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compaction.clone())
    };
    let Some(compaction) = compaction else {
        return Ok(None);
    };

    let snapshot = compaction.snapshot().await;
    if !snapshot.blocks_workflow() {
        return Ok(None);
    }

    let Some(compact_event) = compaction.current_request().await else {
        return Ok(None);
    };

    let current_retry_count = compaction.summary_retry_count().await;
    if current_retry_count >= MAX_COMPACTION_SUMMARY_RETRY_ATTEMPTS {
        return Ok(None);
    }

    let retry_count = compaction.increment_summary_retry_count().await;
    log::warn!(
        "🔁 Retrying compaction summary request after validation failure: session={}, retry_count={}, error={}",
        session_id,
        retry_count,
        validation_error
    );
    emit_compact_request(app_handle, compact_event).await?;
    Ok(Some(CompactResponseOutcome { retried: true }))
}

/// Empty-delta defense: clear in-flight compaction without rewriting compact
/// context, emit Failed so UI/telemetry do not count a success, then resume.
pub(super) async fn abort_empty_delta_compaction_and_resume(
    context: CompactSummaryPersistenceContext<'_>,
) -> Result<CompactResponseOutcome, String> {
    let CompactSummaryPersistenceContext {
        data,
        app_handle,
        session_repo,
        proxy_manager,
    } = context;
    let active_sessions = data.active_sessions;
    let session_id = data.session_id;
    let to_id = data.to_id;
    let session_name = data.session_name;
    let compacted_delta_count = data.compacted_delta_count;

    log::warn!(
        "⏭️ Aborting empty-delta compaction without persist: session={}, to_id={}, compacted_delta_count={}",
        session_id,
        to_id,
        compacted_delta_count
    );

    let resume_action = {
        let compaction = {
            let active = active_sessions.read().await;
            active
                .get(session_id)
                .map(|session| session.compaction.clone())
        };
        if let Some(compaction) = compaction {
            compaction
                .push_telemetry_event(CompactionTelemetryEventDraft {
                    phase: CompactionTelemetryPhase::Failed,
                    to_id: Some(to_id.clone()),
                    condensed_count: Some(0),
                    error: Some(
                        crate::agent::llm::completion::compaction::EMPTY_DELTA_NOOP_ERROR
                            .to_string(),
                    ),
                    epoch_path: None,
                    fallback_path: None,
                    raw_output_path: None,
                    prompt_tokens_before: None,
                    prompt_tokens_after_projection: None,
                })
                .await;
            // complete_success clears request + returns ResumeCompletion for preflight
            // without treating this as a persisted handoff rewrite.
            Some(compaction.complete_success().await)
        } else {
            None
        }
    };
    let resume_action = resume_action.unwrap_or(CompactionResumeAction::Nothing);

    // Still emit Failed so UI clears compacting/awaiting flags; frontend silences
    // the toast for EMPTY_DELTA_NOOP_ERROR (intentional skip, not a user error).
    let dispatcher = TauriEventDispatcher::from_handle(app_handle);
    if let Err(error) = dispatcher.emit_compact_finished(
        session_id.to_string(),
        session_name,
        CompactStatePhase::Failed,
        Some(crate::agent::llm::completion::compaction::EMPTY_DELTA_NOOP_ERROR.to_string()),
    ) {
        log::warn!(
            "Failed to emit compact finished state for session {} after empty-delta abort: {}",
            session_id,
            error
        );
    }

    let resume_repo = session_repo.clone();
    let resume_active_sessions = active_sessions.clone();
    let resume_proxy_manager = proxy_manager.clone();
    let resume_app_handle = app_handle.clone();
    let resume_session_id = session_id.to_string();
    crate::agent::workflow::continue_pending_after_compaction(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
        resume_action,
        || {
            spawn_resume_completion(
                &resume_repo,
                &resume_active_sessions,
                &resume_proxy_manager,
                &resume_app_handle,
                &resume_session_id,
                "LLM completion after empty-delta compaction abort",
            );
        },
    )
    .await?;

    Ok(CompactResponseOutcome { retried: false })
}

pub(super) async fn persist_compact_summary_and_resume(
    context: CompactSummaryPersistenceContext<'_>,
    summary: String,
) -> Result<CompactResponseOutcome, String> {
    let CompactSummaryPersistenceContext {
        data,
        app_handle,
        session_repo,
        proxy_manager,
    } = context;
    let runtime = TauriCompactPersistenceRuntime {
        session_repo,
        active_sessions: data.active_sessions,
        proxy_manager,
        app_handle,
    };
    let hook_dispatcher = crate::services::hooks::execute::DefaultHookDispatcher;
    persist_compact_summary_and_resume_with_runtime(data, &runtime, &hook_dispatcher, summary).await
}

pub(super) async fn persist_compact_summary_and_resume_with_runtime(
    context: CompactSummaryPersistenceData<'_>,
    runtime: &dyn CompactPersistenceRuntime,
    hook_dispatcher: &dyn crate::services::hooks::execute::HookDispatcher,
    summary: String,
) -> Result<CompactResponseOutcome, String> {
    let record = CompactContextRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: context.session_id.to_string(),
        to_id: context.to_id.clone(),
        condensed_count: Some(context.compacted_delta_count),
        summary,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    invalidate_retained_tail_prompt_token_checkpoints(
        context.active_sessions,
        context.session_id,
        &context.to_id,
    )
    .await?;
    save_compact_context(context.active_sessions, context.session_id, record).await?;

    // Drop stale todos before resume Think so handoff is the post-compact
    // residue SSOT. Goal and scratchpad are retained.
    crate::agent::planning_reset::clear_todos_after_successful_compaction(context.session_id).await;

    {
        let compaction = {
            let active = context.active_sessions.read().await;
            active
                .get(context.session_id)
                .map(|session| session.compaction.clone())
        };
        if let Some(compaction) = compaction {
            compaction
                .push_telemetry_event(CompactionTelemetryEventDraft {
                    phase: context.telemetry_phase,
                    to_id: Some(context.to_id.clone()),
                    condensed_count: Some(context.compacted_delta_count),
                    error: None,
                    epoch_path: context.epoch_path.clone(),
                    fallback_path: context.fallback_path.clone(),
                    raw_output_path: context.raw_output_path.clone(),
                    prompt_tokens_before: None,
                    prompt_tokens_after_projection: None,
                })
                .await;
        }
    }

    let resume_action = {
        let active = context.active_sessions.read().await;
        if let Some(session) = active.get(context.session_id) {
            Some(session.compaction.complete_success().await)
        } else {
            None
        }
    };
    let resume_action = resume_action.unwrap_or(CompactionResumeAction::Nothing);

    log::info!(
        "📌 Compact completion decision for session {}: action={}",
        context.session_id,
        match &resume_action {
            CompactionResumeAction::Nothing => "nothing",
            CompactionResumeAction::ResumeCompletion => "resume_completion",
        }
    );
    if let Err(error) = runtime.emit_compact_finished(
        context.session_id.to_string(),
        context.session_name,
        CompactStatePhase::Succeeded,
        None,
    ) {
        log::warn!(
            "Failed to emit compact finished state for session {} after successful compaction: {}",
            context.session_id,
            error
        );
    }

    // #3007: the `PostCompact` matcher value is the compaction trigger
    // ("auto" / "manual"), matching the `PreCompact` convention: a resumed
    // completion means the compaction was auto-triggered by context
    // pressure, a bare settle means the user asked for it. Computed here
    // because `resume_action` is moved into `continue_pending_after_compaction_with`.
    let post_compact_trigger = match &resume_action {
        CompactionResumeAction::ResumeCompletion => "auto",
        CompactionResumeAction::Nothing => "manual",
    };
    crate::agent::workflow::compact_pending::continue_pending_after_compaction_with(
        context.active_sessions,
        context.session_id,
        resume_action,
        || {
            runtime.spawn_resume_completion(context.session_id, "LLM completion");
        },
        || runtime.start_workflow_from_pending_queue(context.session_id),
    )
    .await?;

    hook_dispatcher.spawn_session_start(
        context.session_id,
        crate::services::hooks::SessionStartSource::Compact,
    );

    // #3007 / #3071 G1.4: production emission site for PostCompact.
    let mut post_compact_params = crate::services::hooks::HookEnvelopeParams::new(
        context.session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::PostCompact,
    );
    post_compact_params.compact_source = Some(post_compact_trigger.to_string());
    post_compact_params.compact_trigger = Some(post_compact_trigger.to_string());
    let _ = hook_dispatcher
        .dispatch(
            crate::services::hooks::HookEvent::PostCompact,
            post_compact_trigger,
            &post_compact_params,
            context.session_id,
        )
        .await;

    Ok(CompactResponseOutcome { retried: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::lifecycle::creation::test_support::init_isolated_test_env;
    use crate::agent::state::CompactionKind;
    use crate::repositories::{SessionMetadata, SqliteSessionRepository};
    use crate::services::hooks::execute::RecordingHookDispatcher;
    use crate::services::hooks::HookEvent;

    #[derive(Default)]
    struct RecordingCompactRuntime {
        finished_phases: std::sync::Mutex<Vec<CompactStatePhase>>,
        resumed_sessions: std::sync::Mutex<Vec<String>>,
        started_pending_sessions: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl CompactPersistenceRuntime for RecordingCompactRuntime {
        fn emit_compact_finished(
            &self,
            _session_id: String,
            _session_name: Option<String>,
            phase: CompactStatePhase,
            _error: Option<String>,
        ) -> Result<(), String> {
            self.finished_phases
                .lock()
                .expect("finished_phases lock")
                .push(phase);
            Ok(())
        }

        fn spawn_resume_completion(&self, session_id: &str, _log_label: &'static str) {
            self.resumed_sessions
                .lock()
                .expect("resumed_sessions lock")
                .push(session_id.to_string());
        }

        async fn start_workflow_from_pending_queue(&self, session_id: &str) -> Result<(), String> {
            self.started_pending_sessions
                .lock()
                .expect("started_pending_sessions lock")
                .push(session_id.to_string());
            Ok(())
        }
    }

    async fn run_compaction_with_kind(
        kind: CompactionKind,
        expected_trigger: &str,
        expect_resume: bool,
    ) {
        let _lock = crate::state::lock_test_global_state().await;
        let env = init_isolated_test_env().await;
        let session_repo: Arc<dyn SessionRepository> =
            Arc::new(SqliteSessionRepository::new(env.db.as_ref().clone()));

        let sid = format!("compact-hook-{}-{}", expected_trigger, uuid::Uuid::new_v4());
        let session = SessionMetadata::test_fixture(&sid);
        session_repo
            .upsert_session(&session)
            .await
            .expect("insert test session");
        let agent_session = AgentSession::new(session, Arc::new(ContextRegistry::new()), None);
        agent_session
            .compaction
            .try_begin(kind, Some("msg-1".to_string()), 1_000)
            .await;

        let active_sessions = Arc::new(RwLock::new(HashMap::from([(sid.clone(), agent_session)])));
        let runtime = RecordingCompactRuntime::default();
        let hooks = RecordingHookDispatcher::new();

        persist_compact_summary_and_resume_with_runtime(
            CompactSummaryPersistenceData {
                active_sessions: &active_sessions,
                session_id: &sid,
                session_name: Some("compact-test".to_string()),
                to_id: "msg-1".to_string(),
                compacted_delta_count: 1,
                telemetry_phase: CompactionTelemetryPhase::Succeeded,
                epoch_path: None,
                fallback_path: None,
                raw_output_path: None,
            },
            &runtime,
            &hooks,
            "compacted summary".to_string(),
        )
        .await
        .expect("persist_compact_summary_and_resume should succeed without AppHandle");

        assert!(matches!(
            runtime
                .finished_phases
                .lock()
                .expect("finished_phases")
                .as_slice(),
            [CompactStatePhase::Succeeded]
        ));
        assert_eq!(
            !runtime
                .resumed_sessions
                .lock()
                .expect("resumed_sessions")
                .is_empty(),
            expect_resume
        );

        let observed = hooks.events_for_session(&sid);
        assert!(
            observed.iter().any(|entry| {
                entry.event == HookEvent::PostCompact && entry.matcher_value == expected_trigger
            }),
            "expected PostCompact with trigger '{expected_trigger}', observed: {observed:?}"
        );
    }

    #[tokio::test]
    async fn persist_compact_summary_and_resume_dispatches_post_compact_for_auto_and_manual() {
        run_compaction_with_kind(CompactionKind::Preflight, "auto", true).await;
        run_compaction_with_kind(CompactionKind::Manual, "manual", false).await;
    }
}

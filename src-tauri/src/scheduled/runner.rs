//! Scheduled task runner.
//!
//! For each due task:
//!  1. Load the assistant to build `AgentConfig`
//!  2. Look up (or create) the reused session
//!  3. Check if the session is busy — skip silently if so
//!  4. Inject the task message as a user turn and trigger the workflow
//!  5. Record the run and compute the next fire time

use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use chrono::TimeZone;
use cron::Schedule;
use tauri::AppHandle;
use uuid::Uuid;

use crate::agent::{AgentConfig, AgentSessionManager};
use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};
use crate::repositories::{
    AssistantRepository, ScheduledTaskRepository, SessionRepository, UpdateScheduledTaskParams,
};
use crate::scheduled::{is_one_shot_task, is_session_task, ScheduleTimezone};
use crate::services::WorkspaceService;
use crate::state::{
    get_active_sessions, get_assistant_repository, get_scheduled_task_repository,
    get_session_repository,
};
use tauri::Manager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSessionResolution {
    ReuseActive(String),
    ResumePersisted(String),
    Create(String),
}

/// Decide how a scheduled task should obtain its execution session.
///
/// Resolution order:
/// 1. Reuse the session immediately if it is already active in memory.
/// 2. Resume the exact pinned session if it exists in the persistent repository.
/// 3. Otherwise create a session. When a task already has a pinned session ID,
///    keep using that same ID so pinning remains stable.
pub async fn resolve_task_session_resolution(
    task_session_id: Option<&str>,
    active_session_ids: &HashSet<String>,
    session_repo: &dyn SessionRepository,
) -> Result<TaskSessionResolution, String> {
    let Some(session_id) = task_session_id else {
        return Ok(TaskSessionResolution::Create(Uuid::new_v4().to_string()));
    };

    if active_session_ids.contains(session_id) {
        return Ok(TaskSessionResolution::ReuseActive(session_id.to_string()));
    }

    if session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load pinned session {session_id}: {e}"))?
        .is_some()
    {
        return Ok(TaskSessionResolution::ResumePersisted(
            session_id.to_string(),
        ));
    }

    Ok(TaskSessionResolution::Create(session_id.to_string()))
}

fn is_stale_workspace_override(path: &str) -> bool {
    !Path::new(path).is_dir()
}

pub async fn sync_task_workspace_override(
    repo: &dyn ScheduledTaskRepository,
    task_id: &str,
    task_name: &str,
    session_id: &str,
    workspace_override: Option<&str>,
) -> Result<(), String> {
    if let Some(path) = workspace_override {
        if is_stale_workspace_override(path) {
            log::warn!(
                "⏰ Scheduled task '{}' ({}) references stale workspace override '{}'; \
                 clearing override and falling back to the default workspace.",
                task_name,
                task_id,
                path
            );
            WorkspaceService::cancel_override(session_id).await?;
            repo.update_scheduled_task(
                task_id,
                UpdateScheduledTaskParams {
                    name: None,
                    cron_expression: None,
                    schedule_timezone: None,
                    assistant_id: None,
                    group_id: None,
                    group_name: None,
                    message: None,
                    yolo_mode: None,
                    workspace_override: Some(None),
                    enabled: None,
                    next_run_at: None,
                },
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to clear stale workspace override for scheduled task {}: {}",
                    task_id, e
                )
            })?;
            Ok(())
        } else {
            WorkspaceService::set_override(session_id, path.to_string()).await
        }
    } else {
        WorkspaceService::cancel_override(session_id).await
    }
}

/// Execute all tasks whose `next_run_at <= now_ms`.
pub async fn execute_due_tasks(app_handle: &AppHandle, now_ms: i64) -> Result<(), String> {
    let repo = get_scheduled_task_repository();
    let due = repo
        .list_due_tasks(now_ms)
        .await
        .map_err(|e| format!("Failed to load due tasks: {e}"))?;

    if due.is_empty() {
        return Ok(());
    }

    let manager: tauri::State<'_, AgentSessionManager> = app_handle.state();

    for task in due {
        if let Err(e) = execute_task(&manager, &task, now_ms).await {
            log::error!(
                "⏰ Failed to execute scheduled task '{}' ({}): {}",
                task.name,
                task.id,
                e
            );
        }
    }

    Ok(())
}

async fn execute_task(
    manager: &AgentSessionManager,
    task: &crate::entity::scheduled_task::Model,
    now_ms: i64,
) -> Result<(), String> {
    if is_session_task(&task.task_category) {
        execute_session_callback(manager, task, now_ms).await
    } else {
        execute_global_task(manager, task, now_ms).await
    }
}

async fn execute_global_task(
    manager: &AgentSessionManager,
    task: &crate::entity::scheduled_task::Model,
    now_ms: i64,
) -> Result<(), String> {
    let cron_expression = task.cron_expression.as_deref().ok_or_else(|| {
        format!(
            "GLOBAL task '{}' ({}) is missing a cron expression",
            task.name, task.id
        )
    })?;

    // ── 1. Build AgentConfig from stored assistant ────────────────────────────
    let assistant = get_assistant_repository()
        .get_assistant(&task.assistant_id)
        .await
        .map_err(|e| format!("DB error loading assistant: {e}"))?
        .ok_or_else(|| format!("Assistant {} not found", task.assistant_id))?;

    let agent_config = AgentConfig::from_json(&assistant.config)?;

    let agent_config = AgentConfig {
        id: Some(task.assistant_id.clone()),
        name: assistant.name.clone(),
        ..agent_config
    };

    // ── 2. Resolve session (create on first run OR after session loss) ──────────
    let active_sessions = get_active_sessions();

    let active_session_ids = {
        let sessions = active_sessions.read().await;
        sessions.keys().cloned().collect::<HashSet<_>>()
    };
    let session_repo = get_session_repository();
    let resolution = resolve_task_session_resolution(
        task.session_id.as_deref(),
        &active_session_ids,
        session_repo,
    )
    .await?;

    let (session_id, is_new_session) = match resolution {
        TaskSessionResolution::ReuseActive(sid) => {
            if let Err(e) = manager.set_yolo_mode(&sid, task.yolo_mode).await {
                log::warn!(
                    "⏰ Failed to sync YOLO mode for existing session {}: {}",
                    sid,
                    e
                );
            }
            (sid, false)
        }
        TaskSessionResolution::ResumePersisted(sid) => {
            manager.resume_session(&sid).await?;
            if let Err(e) = manager.set_yolo_mode(&sid, task.yolo_mode).await {
                log::warn!(
                    "⏰ Failed to sync YOLO mode for resumed session {}: {}",
                    sid,
                    e
                );
            }
            (sid, false)
        }
        TaskSessionResolution::Create(sid) => {
            let task_name = format!("⏰ {}", task.name);
            manager
                .create_session(
                    sid.clone(),
                    Some(task_name),
                    None,
                    None,
                    agent_config.clone(),
                )
                .await?;

            if task.yolo_mode {
                if let Err(e) = manager.set_yolo_mode(&sid, true).await {
                    log::warn!("⏰ Failed to set YOLO mode for new session {}: {}", sid, e);
                }
            }
            (sid, true)
        }
    };

    if is_session_busy(&active_sessions, &session_id, &task.name, task.id.as_str()).await {
        let next_run_at = compute_next_run_for_schedule_timezone(
            cron_expression,
            now_ms,
            &task.schedule_timezone,
        )?;
        let repo = get_scheduled_task_repository();
        repo.record_run(&task.id, None, now_ms, next_run_at)
            .await
            .map_err(|e| format!("Failed to record skipped run: {e}"))?;
        return Ok(());
    }

    let repo = get_scheduled_task_repository();
    sync_task_workspace_override(
        repo,
        &task.id,
        &task.name,
        &session_id,
        task.workspace_override.as_deref(),
    )
    .await?;

    inject_scheduled_message(manager, &session_id, task).await?;

    let next_run_at =
        compute_next_run_for_schedule_timezone(cron_expression, now_ms, &task.schedule_timezone)?;
    let new_session_id = is_new_session.then_some(session_id);
    repo.record_run(&task.id, new_session_id, now_ms, next_run_at)
        .await
        .map_err(|e| format!("Failed to record run: {e}"))?;

    log::info!("⏰ Triggered scheduled task '{}'", task.name);
    Ok(())
}

async fn execute_session_callback(
    manager: &AgentSessionManager,
    task: &crate::entity::scheduled_task::Model,
    now_ms: i64,
) -> Result<(), String> {
    let session_id = task.session_id.as_deref().ok_or_else(|| {
        format!(
            "SESSION task '{}' ({}) is missing a pinned session_id",
            task.name, task.id
        )
    })?;

    let active_sessions = get_active_sessions();
    let active_session_ids = {
        let sessions = active_sessions.read().await;
        sessions.keys().cloned().collect::<HashSet<_>>()
    };
    let session_repo = get_session_repository();

    let session_exists_in_repo = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load session {session_id}: {e}"))?
        .is_some();

    if !active_session_ids.contains(session_id) && !session_exists_in_repo {
        let repo = get_scheduled_task_repository();
        repo.update_scheduled_task(
            &task.id,
            UpdateScheduledTaskParams {
                enabled: Some(false),
                next_run_at: Some(None),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to disable orphaned SESSION task {}: {e}", task.id))?;
        log::warn!(
            "⏰ SESSION task '{}' ({}) disabled — target session {} no longer exists",
            task.name,
            task.id,
            session_id
        );
        return Ok(());
    }

    if !active_session_ids.contains(session_id) {
        manager.resume_session(session_id).await?;
    }

    if let Err(e) = manager.set_yolo_mode(session_id, task.yolo_mode).await {
        log::warn!(
            "⏰ Failed to sync YOLO mode for session callback {}: {}",
            session_id,
            e
        );
    }

    if is_session_busy(&active_sessions, session_id, &task.name, task.id.as_str()).await {
        if is_one_shot_task(&task.cron_expression) {
            log::info!(
                "⏰ SESSION one-shot '{}' ({}) skipped — session {} is busy; \
                 will retry on the next scheduler tick",
                task.name,
                task.id,
                session_id
            );
        } else {
            let cron_expression = task.cron_expression.as_deref().ok_or_else(|| {
                format!(
                    "SESSION recurring task '{}' ({}) is missing a cron expression",
                    task.name, task.id
                )
            })?;
            let next_run_at = compute_next_run_for_schedule_timezone(
                cron_expression,
                now_ms,
                &task.schedule_timezone,
            )?;
            let repo = get_scheduled_task_repository();
            repo.record_run(&task.id, None, now_ms, next_run_at)
                .await
                .map_err(|e| format!("Failed to record skipped SESSION run: {e}"))?;
        }
        return Ok(());
    }

    let repo = get_scheduled_task_repository();
    sync_task_workspace_override(
        repo,
        &task.id,
        &task.name,
        session_id,
        task.workspace_override.as_deref(),
    )
    .await?;

    inject_scheduled_message(manager, session_id, task).await?;

    if is_one_shot_task(&task.cron_expression) {
        repo.record_run(&task.id, None, now_ms, None)
            .await
            .map_err(|e| format!("Failed to record one-shot SESSION run: {e}"))?;
        repo.update_scheduled_task(
            &task.id,
            UpdateScheduledTaskParams {
                enabled: Some(false),
                next_run_at: Some(None),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to disable one-shot SESSION task {}: {e}", task.id))?;
    } else {
        let cron_expression = task.cron_expression.as_deref().ok_or_else(|| {
            format!(
                "SESSION recurring task '{}' ({}) is missing a cron expression",
                task.name, task.id
            )
        })?;
        let next_run_at = compute_next_run_for_schedule_timezone(
            cron_expression,
            now_ms,
            &task.schedule_timezone,
        )?;
        repo.record_run(&task.id, None, now_ms, next_run_at)
            .await
            .map_err(|e| format!("Failed to record SESSION run: {e}"))?;
    }

    log::info!("⏰ Triggered SESSION callback '{}'", task.name);
    Ok(())
}

async fn is_session_busy(
    active_sessions: &std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, crate::agent::state::AgentSession>>>,
    session_id: &str,
    task_name: &str,
    task_id: &str,
) -> bool {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        if session.is_running {
            log::info!(
                "⏰ Skipping task '{}' ({}) — session {} is busy",
                task_name,
                task_id,
                session_id
            );
            return true;
        }
    }
    false
}

async fn inject_scheduled_message(
    manager: &AgentSessionManager,
    session_id: &str,
    task: &crate::entity::scheduled_task::Model,
) -> Result<(), String> {
    let now_ts = chrono::Utc::now().timestamp_millis();
    let user_message = Message {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: task.message.clone(),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: Some(task.assistant_id.clone()),
        usage: None,
        prompt_tokens: None,
        attachments: None,
        tool_use: None,
        created_at: now_ts,
        updated_at: now_ts,
        source: Some(MessageSource::ScheduledTask),
        error: None,
        metadata: None,
    };

    manager
        .inject_messages(session_id.to_string(), vec![user_message])
        .await?;
    Ok(())
}

/// Compute the next epoch-ms fire time from a cron expression using the given timezone.
/// Returns `None` if the expression is invalid or has no future occurrences.
pub fn compute_next_run_for_timezone<Tz: TimeZone>(
    cron_expression: &str,
    reference_ms: i64,
    timezone: Tz,
) -> Option<i64> {
    let normalized = super::normalize_cron(cron_expression);
    let schedule = Schedule::from_str(&normalized).ok()?;

    let after = timezone
        .timestamp_millis_opt(reference_ms + 1000)
        .single()?;

    schedule
        .after(&after)
        .next()
        .map(|dt| dt.timestamp_millis())
}

/// Compute the next fire time from a cron expression using the task's schedule timezone.
pub fn compute_next_run_for_schedule_timezone(
    cron_expression: &str,
    reference_ms: i64,
    schedule_timezone: &str,
) -> Result<Option<i64>, String> {
    let timezone = ScheduleTimezone::parse(schedule_timezone)?;
    let next_run = match timezone {
        ScheduleTimezone::Utc => {
            compute_next_run_for_timezone(cron_expression, reference_ms, chrono::Utc)
        }
        ScheduleTimezone::Local => {
            compute_next_run_for_timezone(cron_expression, reference_ms, chrono::Local)
        }
    };

    Ok(next_run)
}

/// Compute the next local-time epoch-ms fire time from a cron expression.
///
/// Scheduled tasks are user-facing calendar schedules, so daily/weekly/monthly
/// rules are interpreted in the machine's local timezone instead of UTC.
pub fn compute_next_run(cron_expression: &str, reference_ms: i64) -> Option<i64> {
    compute_next_run_for_timezone(cron_expression, reference_ms, chrono::Local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_next_run_accuracy() {
        let cron = "0 * * * * * *";

        let ref_ms = 1740988800000;
        let next = compute_next_run_for_timezone(cron, ref_ms, chrono::Utc).unwrap();
        assert_eq!(next, ref_ms + 60000);

        let ref_ms = 1740988799000;
        let next = compute_next_run_for_timezone(cron, ref_ms, chrono::Utc).unwrap();
        assert_eq!(next, 1740988860000);

        let ref_ms = 1740988770000;
        let next = compute_next_run_for_timezone(cron, ref_ms, chrono::Utc).unwrap();
        assert_eq!(next, 1740988800000);
    }
}

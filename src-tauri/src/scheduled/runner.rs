//! Scheduled task runner.
//!
//! For each due task:
//!  1. Load the assistant to build `AgentConfig`
//!  2. Look up (or create) the reused session
//!  3. Check if the session is busy — skip silently if so
//!  4. Inject the task message as a user turn and trigger the workflow
//!  5. Record the run and compute the next fire time

use std::str::FromStr;

use cron::Schedule;
use tauri::AppHandle;
use uuid::Uuid;

use crate::agent::{AgentConfig, AgentSessionManager};
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::{AssistantRepository, ScheduledTaskRepository};
use crate::state::{get_active_sessions, get_assistant_repository, get_scheduled_task_repository};
use tauri::Manager;

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
    // ── 1. Build AgentConfig from stored assistant ────────────────────────────
    let assistant = get_assistant_repository()
        .get_assistant(&task.assistant_id)
        .await
        .map_err(|e| format!("DB error loading assistant: {e}"))?
        .ok_or_else(|| format!("Assistant {} not found", task.assistant_id))?;

    let agent_config = AgentConfig::from_json(&assistant.config)?;

    // Ensure the assistant ID and name are embedded in the config.
    // The assistant entity stores `id` and `name` as separate DB columns from `config`,
    // so the config JSON may not contain the correct values on its own.
    // In particular, `name` defaults to "Unknown Assistant" if not set in the config JSON.
    let agent_config = AgentConfig {
        id: Some(task.assistant_id.clone()),
        name: assistant.name.clone(),
        ..agent_config
    };

    // ── 2. Resolve session (create on first run OR after session loss) ──────────
    // A stored session_id may be stale after an app restart or session cleanup.
    // Check active_sessions first; recreate if missing.
    let active_sessions = get_active_sessions();

    let session_exists = if let Some(ref sid) = task.session_id {
        active_sessions.read().await.contains_key(sid.as_str())
    } else {
        false
    };

    let (session_id, is_new_session) = if session_exists {
        // Reuse the live session
        (task.session_id.clone().unwrap(), false)
    } else {
        // First run OR session was lost — create a fresh one
        let new_id = Uuid::new_v4().to_string();
        let task_name = format!("⏰ {}", task.name);
        manager
            .create_session(
                new_id.clone(),
                Some(task_name),
                None,
                None,
                agent_config.clone(),
            )
            .await?;
        (new_id, true)
    };

    // ── 3. Skip if session is currently running ───────────────────────────────
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            if session.is_running {
                log::info!(
                    "⏰ Skipping task '{}' — session {} is busy",
                    task.name,
                    session_id
                );
                return Ok(());
            }
        }
    }

    // ── 4. Inject message and trigger workflow ────────────────────────────────
    let now_ts = chrono::Utc::now().timestamp_millis();
    let user_message = Message {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
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
        attachments: None,
        tool_use: None,
        created_at: now_ts,
        updated_at: now_ts,
        source: Some("scheduled_task".to_string()),
        error: None,
        metadata: None,
    };

    manager
        .inject_messages(session_id.clone(), vec![user_message], true)
        .await?;

    // ── 5. Record the run and schedule the next fire time ─────────────────────
    let next_run_at = compute_next_run(&task.cron_expression);
    let repo = get_scheduled_task_repository();
    let new_session_id = is_new_session.then_some(session_id);
    repo.record_run(&task.id, new_session_id, now_ms, next_run_at)
        .await
        .map_err(|e| format!("Failed to record run: {e}"))?;

    log::info!("⏰ Triggered scheduled task '{}'", task.name);
    Ok(())
}

/// Compute the next UTC epoch-ms fire time from a cron expression.
/// Returns `None` if the expression is invalid or has no future occurrences.
pub fn compute_next_run(cron_expression: &str) -> Option<i64> {
    let normalized = super::normalize_cron(cron_expression);
    let schedule = Schedule::from_str(&normalized).ok()?;
    // Use now + 60 s (the worker's tick interval) as the reference so that
    // the returned next_run_at is always at least one full tick in the future.
    // This prevents double-fires when a task triggers close to a cron boundary
    // (e.g. */10 fires at :59 → without the offset the next boundary :00 would
    // be only ~44 s away and the worker would pick it up on the very next tick).
    let after = chrono::Utc::now() + chrono::Duration::seconds(60);
    schedule
        .after(&after)
        .next()
        .map(|dt| dt.timestamp_millis())
}

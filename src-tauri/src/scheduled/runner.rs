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
        let sid = task.session_id.clone().unwrap();
        // Ensure YOLO mode is synced even for reused sessions
        if let Err(e) = manager.set_yolo_mode(&sid, task.yolo_mode).await {
            log::warn!(
                "⏰ Failed to sync YOLO mode for existing session {}: {}",
                sid,
                e
            );
        }
        (sid, false)
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

        // Apply YOLO mode from task to the new session
        if task.yolo_mode {
            if let Err(e) = manager.set_yolo_mode(&new_id, true).await {
                log::warn!(
                    "⏰ Failed to set YOLO mode for new session {}: {}",
                    new_id,
                    e
                );
            }
        }
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
                // Record the skip so we don't hot-loop (reschedule for next occurrence)
                let next_run_at = compute_next_run(&task.cron_expression, now_ms);
                let repo = get_scheduled_task_repository();
                repo.record_run(&task.id, None, now_ms, next_run_at)
                    .await
                    .map_err(|e| format!("Failed to record skipped run: {e}"))?;
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
        usage: None,
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
    let next_run_at = compute_next_run(&task.cron_expression, now_ms);
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
pub fn compute_next_run(cron_expression: &str, reference_ms: i64) -> Option<i64> {
    let normalized = super::normalize_cron(cron_expression);
    let schedule = Schedule::from_str(&normalized).ok()?;

    // Use reference_ms + 1s as the baseline for the next occurrence.
    // This provides a tiny epsilon to prevent double-firing on the same tick,
    // while remaining much more accurate than the previous 60s hardcoded offset.
    let after = chrono::DateTime::from_timestamp_millis(reference_ms + 1000)?;

    schedule
        .after(&after)
        .next()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_next_run_accuracy() {
        // Cron: every minute at 0 seconds
        let cron = "0 * * * * * *";

        // 1. Reference is exactly at boundary :00 (12:00:00)
        let ref_ms = 1740988800000; // 2025-03-03 12:00:00 UTC
        let next = compute_next_run(cron, ref_ms).unwrap();
        // Should be 12:01:00 (ref + 1s buffer makes it look after 12:00:01)
        assert_eq!(next, ref_ms + 60000);

        // 2. Reference is just before boundary :59 (11:59:59)
        let ref_ms = 1740988799000;
        let next = compute_next_run(cron, ref_ms).unwrap();
        // Should be 12:01:00 (ref + 1s buffer makes it 12:00:00, .after() takes us to 12:01:00)
        assert_eq!(next, 1740988860000);

        // 3. Reference is well before boundary (11:59:30)
        let ref_ms = 1740988770000;
        let next = compute_next_run(cron, ref_ms).unwrap();
        // Should be 12:00:00
        assert_eq!(next, 1740988800000);
    }
}

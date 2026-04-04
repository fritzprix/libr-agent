use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use serde_json::{json, Value};

pub fn render_task_line(task: &ScheduledTaskModel) -> String {
    format!(
        "- {} | {} | {} | next: {} | assistant: {}",
        task.id,
        task.name,
        if task.enabled { "enabled" } else { "disabled" },
        format_timestamp(task.next_run_at),
        task.assistant_id
    )
}

pub fn render_task_detail(task: &ScheduledTaskModel) -> String {
    let workspace_override = task
        .workspace_override
        .as_deref()
        .unwrap_or("none")
        .to_string();
    let pinned_session = task.session_id.as_deref().unwrap_or("none").to_string();

    format!(
        "Scheduled task {}\n\n\
Name: {}\n\
Assistant: {}\n\
Enabled: {}\n\
Cron: {}\n\
Timezone: {}\n\
YOLO mode: {}\n\
Next run: {}\n\
Last run: {}\n\
Pinned session: {}\n\
Workspace override: {}\n\n\
Message:\n{}",
        task.id,
        task.name,
        task.assistant_id,
        if task.enabled { "yes" } else { "no" },
        task.cron_expression,
        task.schedule_timezone,
        if task.yolo_mode { "yes" } else { "no" },
        format_timestamp(task.next_run_at),
        format_timestamp(task.last_run_at),
        pinned_session,
        workspace_override,
        task.message
    )
}

pub fn task_to_json(task: &ScheduledTaskModel) -> Value {
    json!({
        "id": task.id,
        "name": task.name,
        "cronExpression": task.cron_expression,
        "scheduleTimezone": task.schedule_timezone,
        "assistantId": task.assistant_id,
        "message": task.message,
        "yoloMode": task.yolo_mode,
        "sessionId": task.session_id,
        "workspaceOverride": task.workspace_override,
        "enabled": task.enabled,
        "lastRunAt": task.last_run_at,
        "nextRunAt": task.next_run_at,
        "createdAt": task.created_at,
        "updatedAt": task.updated_at
    })
}

pub fn format_timestamp(timestamp_ms: Option<i64>) -> String {
    let Some(timestamp_ms) = timestamp_ms else {
        return "not scheduled".to_string();
    };

    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_ms)
        .map(|value| value.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| timestamp_ms.to_string())
}

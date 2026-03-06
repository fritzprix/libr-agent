//! Tauri commands for managing scheduled tasks.

use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::services::ScheduledTaskService;
use serde::{Deserialize, Serialize};
use tauri::command;

/// Data-transfer object for scheduled tasks returned to the frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskDto {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub assistant_id: String,
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
    pub session_id: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ScheduledTaskModel> for ScheduledTaskDto {
    fn from(m: ScheduledTaskModel) -> Self {
        Self {
            id: m.id,
            name: m.name,
            cron_expression: m.cron_expression,
            assistant_id: m.assistant_id,
            message: m.message,
            session_id: m.session_id,
            enabled: m.enabled,
            last_run_at: m.last_run_at,
            next_run_at: m.next_run_at,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

/// Request to create a new scheduled task
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    pub cron_expression: String,
    pub assistant_id: String,
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
}

/// Request to update a scheduled task
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScheduledTaskRequest {
    pub name: Option<String>,
    pub cron_expression: Option<String>,
    pub message: Option<String>,
    pub enabled: Option<bool>,
}

/// Create a new scheduled task
#[command]
pub async fn create_scheduled_task(
    request: CreateScheduledTaskRequest,
) -> Result<ScheduledTaskDto, String> {
    ScheduledTaskService::create_scheduled_task(
        request.name,
        request.cron_expression,
        request.assistant_id,
        request.message,
    )
    .await
    .map(ScheduledTaskDto::from)
}

/// List scheduled tasks, optionally filtered by assistant
#[command]
pub async fn list_scheduled_tasks(
    assistant_id: Option<String>,
) -> Result<Vec<ScheduledTaskDto>, String> {
    ScheduledTaskService::list_scheduled_tasks(assistant_id.as_deref())
        .await
        .map(|v| v.into_iter().map(ScheduledTaskDto::from).collect())
}

/// Get a single scheduled task by ID
#[command]
pub async fn get_scheduled_task(id: String) -> Result<Option<ScheduledTaskDto>, String> {
    ScheduledTaskService::get_scheduled_task(&id)
        .await
        .map(|opt| opt.map(ScheduledTaskDto::from))
}

/// Update a scheduled task
#[command]
pub async fn update_scheduled_task(
    id: String,
    request: UpdateScheduledTaskRequest,
) -> Result<ScheduledTaskDto, String> {
    ScheduledTaskService::update_scheduled_task(
        &id,
        request.name,
        request.cron_expression,
        request.message,
        request.enabled,
    )
    .await
    .map(ScheduledTaskDto::from)
}

/// Toggle enabled/disabled state of a scheduled task
#[command]
pub async fn toggle_scheduled_task(id: String, enabled: bool) -> Result<ScheduledTaskDto, String> {
    ScheduledTaskService::toggle_scheduled_task(&id, enabled)
        .await
        .map(ScheduledTaskDto::from)
}

/// Delete a scheduled task
#[command]
pub async fn delete_scheduled_task(id: String) -> Result<(), String> {
    ScheduledTaskService::delete_scheduled_task(&id).await
}

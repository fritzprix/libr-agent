//! Tauri commands for managing scheduled tasks.

use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::UpdateScheduledTaskParams;
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::services::{default_schedule_timezone, CreateScheduledTaskInput, ScheduledTaskService};
use crate::state::get_scheduled_task_repository;
use serde::{Deserialize, Serialize};
use tauri::command;

/// Data-transfer object for scheduled tasks returned to the frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskDto {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
    pub yolo_mode: bool,
    pub created_by_session_id: Option<String>,
    pub session_id: Option<String>,
    pub workspace_override: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ScheduledTaskModel> for ScheduledTaskDto {
    fn from(m: ScheduledTaskModel) -> Self {
        let next_run_at = m.next_run_at.or_else(|| {
            if m.enabled {
                return None;
            }

            compute_next_run_for_schedule_timezone(
                &m.cron_expression,
                chrono::Utc::now().timestamp_millis(),
                &m.schedule_timezone,
            )
            .ok()
            .flatten()
        });

        Self {
            id: m.id,
            name: m.name,
            cron_expression: m.cron_expression,
            schedule_timezone: m.schedule_timezone,
            assistant_id: m.assistant_id,
            group_id: m.group_id,
            group_name: m.group_name,
            message: m.message,
            yolo_mode: m.yolo_mode,
            created_by_session_id: m.created_by_session_id,
            session_id: m.session_id,
            workspace_override: m.workspace_override,
            enabled: m.enabled,
            last_run_at: m.last_run_at,
            next_run_at,
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
    pub schedule_timezone: Option<String>,
    pub assistant_id: String,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
    pub yolo_mode: bool,
    pub workspace_override: Option<String>,
}

/// Request to update a scheduled task
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScheduledTaskRequest {
    pub name: Option<String>,
    pub cron_expression: Option<String>,
    pub schedule_timezone: Option<String>,
    pub assistant_id: Option<String>,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub message: Option<String>,
    pub yolo_mode: Option<bool>,
    pub workspace_override: Option<Option<String>>,
    pub clear_group: Option<bool>,
    pub enabled: Option<bool>,
}

/// Create a new scheduled task
#[command]
pub async fn create_scheduled_task(
    request: CreateScheduledTaskRequest,
) -> Result<ScheduledTaskDto, String> {
    ScheduledTaskService::create_scheduled_task(
        get_scheduled_task_repository(),
        CreateScheduledTaskInput {
            name: request.name,
            cron_expression: request.cron_expression,
            schedule_timezone: request
                .schedule_timezone
                .unwrap_or_else(|| default_schedule_timezone().to_string()),
            assistant_id: request.assistant_id,
            group_id: request.group_id,
            group_name: request.group_name,
            message: request.message,
            yolo_mode: request.yolo_mode,
            created_by_session_id: None,
            workspace_override: request.workspace_override,
        },
    )
    .await
    .map(ScheduledTaskDto::from)
}

/// List scheduled tasks, optionally filtered by assistant
#[command]
pub async fn list_scheduled_tasks(
    assistant_id: Option<String>,
) -> Result<Vec<ScheduledTaskDto>, String> {
    ScheduledTaskService::list_scheduled_tasks(
        get_scheduled_task_repository(),
        assistant_id.as_deref(),
    )
    .await
    .map(|v| v.into_iter().map(ScheduledTaskDto::from).collect())
}

/// Get a single scheduled task by ID
#[command]
pub async fn get_scheduled_task(id: String) -> Result<Option<ScheduledTaskDto>, String> {
    ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &id)
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
        get_scheduled_task_repository(),
        &id,
        UpdateScheduledTaskParams {
            name: request.name,
            cron_expression: request.cron_expression,
            schedule_timezone: request.schedule_timezone,
            assistant_id: request.assistant_id,
            group_id: if request.clear_group.unwrap_or(false) {
                Some(None)
            } else {
                request.group_id.map(Some)
            },
            group_name: if request.clear_group.unwrap_or(false) {
                Some(None)
            } else {
                request.group_name.map(Some)
            },
            message: request.message,
            yolo_mode: request.yolo_mode,
            workspace_override: request.workspace_override,
            enabled: request.enabled,
            next_run_at: None,
        },
    )
    .await
    .map(ScheduledTaskDto::from)
}

/// Toggle enabled/disabled state of a scheduled task
#[command]
pub async fn toggle_scheduled_task(id: String, enabled: bool) -> Result<ScheduledTaskDto, String> {
    ScheduledTaskService::toggle_scheduled_task(get_scheduled_task_repository(), &id, enabled)
        .await
        .map(ScheduledTaskDto::from)
}

/// Delete a scheduled task
#[command]
pub async fn delete_scheduled_task(id: String) -> Result<(), String> {
    ScheduledTaskService::delete_scheduled_task(get_scheduled_task_repository(), &id).await
}

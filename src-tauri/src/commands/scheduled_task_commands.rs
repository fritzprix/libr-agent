//! Tauri commands for managing scheduled tasks.

use crate::agent::ExecutionMode;
use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::UpdateScheduledTaskParams;
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::scheduled::{is_one_shot_task, TASK_CATEGORY_GLOBAL};
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
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
    pub execution_mode: String,
    pub created_by_session_id: Option<String>,
    pub session_id: Option<String>,
    pub task_category: String,
    pub workspace_override: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Session callback shown in the active session schedules panel
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionScheduledTaskDto {
    pub id: String,
    pub name: String,
    pub message: String,
    pub session_id: Option<String>,
    pub is_one_shot: bool,
    pub next_run_at: Option<i64>,
}

impl From<ScheduledTaskModel> for SessionScheduledTaskDto {
    fn from(m: ScheduledTaskModel) -> Self {
        Self {
            id: m.id,
            name: m.name,
            message: m.message,
            session_id: m.session_id,
            is_one_shot: is_one_shot_task(&m.cron_expression),
            next_run_at: m.next_run_at,
        }
    }
}

impl From<ScheduledTaskModel> for ScheduledTaskDto {
    fn from(m: ScheduledTaskModel) -> Self {
        let next_run_at = m.next_run_at.or_else(|| {
            if m.enabled {
                return None;
            }

            m.cron_expression.as_deref().and_then(|cron| {
                compute_next_run_for_schedule_timezone(
                    cron,
                    chrono::Utc::now().timestamp_millis(),
                    &m.schedule_timezone,
                )
                .ok()
                .flatten()
            })
        });

        let execution_mode = m.execution_mode().as_str().to_string();

        Self {
            id: m.id,
            name: m.name,
            cron_expression: m.cron_expression.unwrap_or_default(),
            schedule_timezone: m.schedule_timezone,
            assistant_id: m.assistant_id,
            message: m.message,
            execution_mode,
            created_by_session_id: m.created_by_session_id,
            session_id: m.session_id,
            task_category: m.task_category,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    pub cron_expression: String,
    pub schedule_timezone: Option<String>,
    pub assistant_id: String,
    /// Message text; supports `@playbook:name` and `@skill:name` mention syntax
    pub message: String,
    #[serde(default)]
    pub execution_mode: Option<String>,
    pub workspace_override: Option<String>,
}

/// Request to update a scheduled task
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateScheduledTaskRequest {
    pub name: Option<String>,
    pub cron_expression: Option<String>,
    pub schedule_timezone: Option<String>,
    pub assistant_id: Option<String>,
    pub message: Option<String>,
    pub execution_mode: Option<String>,
    pub workspace_override: Option<Option<String>>,
    pub enabled: Option<bool>,
}

fn parse_execution_mode(mode: Option<&str>) -> Result<ExecutionMode, String> {
    mode.unwrap_or("normal")
        .parse::<ExecutionMode>()
        .map_err(|error| format!("Invalid executionMode: {error}"))
}

/// Create a new scheduled task
#[command]
pub async fn create_scheduled_task(
    request: CreateScheduledTaskRequest,
) -> Result<ScheduledTaskDto, String> {
    let execution_mode = parse_execution_mode(request.execution_mode.as_deref())?;

    ScheduledTaskService::create_scheduled_task(
        get_scheduled_task_repository(),
        CreateScheduledTaskInput {
            name: request.name,
            task_category: crate::scheduled::TASK_CATEGORY_GLOBAL.to_string(),
            cron_expression: Some(request.cron_expression),
            schedule_timezone: request
                .schedule_timezone
                .unwrap_or_else(|| default_schedule_timezone().to_string()),
            assistant_id: request.assistant_id,
            message: request.message,
            execution_mode,
            created_by_session_id: None,
            session_id: None,
            workspace_override: request.workspace_override,
            next_run_at: None,
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
    .map(|v| {
        v.into_iter()
            .filter(|t| t.task_category == TASK_CATEGORY_GLOBAL)
            .map(ScheduledTaskDto::from)
            .collect()
    })
}

/// Get a single scheduled task by ID
#[command]
pub async fn get_scheduled_task(id: String) -> Result<Option<ScheduledTaskDto>, String> {
    ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &id)
        .await
        .map(|opt| {
            opt.filter(|t| t.task_category == TASK_CATEGORY_GLOBAL)
                .map(ScheduledTaskDto::from)
        })
}

/// Update a scheduled task
#[command]
pub async fn update_scheduled_task(
    id: String,
    request: UpdateScheduledTaskRequest,
) -> Result<ScheduledTaskDto, String> {
    let params = UpdateScheduledTaskParams {
        name: request.name,
        cron_expression: request.cron_expression,
        schedule_timezone: request.schedule_timezone,
        assistant_id: request.assistant_id,
        message: request.message,
        execution_mode: request
            .execution_mode
            .map(|mode| parse_execution_mode(Some(mode.as_str())))
            .transpose()?,
        workspace_override: request.workspace_override,
        enabled: request.enabled,
        next_run_at: None,
    };

    ScheduledTaskService::update_scheduled_task(get_scheduled_task_repository(), &id, params)
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

/// List SESSION callbacks pinned to an active session
#[command]
pub async fn list_session_scheduled_tasks(
    session_id: String,
) -> Result<Vec<SessionScheduledTaskDto>, String> {
    ScheduledTaskService::list_session_scheduled_tasks(get_scheduled_task_repository(), &session_id)
        .await
        .map(|tasks| {
            tasks
                .into_iter()
                .map(SessionScheduledTaskDto::from)
                .collect()
        })
}

/// Cancel a SESSION callback from the active session panel
#[command]
pub async fn cancel_session_scheduled_task(
    session_id: String,
    task_id: String,
) -> Result<(), String> {
    ScheduledTaskService::cancel_session_scheduled_task(
        get_scheduled_task_repository(),
        &session_id,
        &task_id,
    )
    .await
}

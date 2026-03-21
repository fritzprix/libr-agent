use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::{
    CreateScheduledTaskParams, ScheduledTaskRepository, UpdateScheduledTaskParams,
};
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::scheduled::{ScheduleTimezone, SCHEDULE_TIMEZONE_LOCAL};
use uuid::Uuid;

pub struct ScheduledTaskService;

pub struct CreateScheduledTaskInput {
    pub name: String,
    pub cron_expression: String,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub message: String,
    pub yolo_mode: bool,
    pub workspace_override: Option<String>,
}

impl ScheduledTaskService {
    pub async fn create_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        input: CreateScheduledTaskInput,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let normalized_schedule_timezone = normalize_schedule_timezone(&input.schedule_timezone)?;
        let next_run_at = compute_next_run_for_schedule_timezone(
            &input.cron_expression,
            now_ms,
            normalized_schedule_timezone,
        )?
        .ok_or_else(|| {
            format!(
                "Invalid cron expression '{}': no future occurrences found",
                input.cron_expression
            )
        })?;
        repo.create_scheduled_task(CreateScheduledTaskParams {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            cron_expression: input.cron_expression,
            schedule_timezone: normalized_schedule_timezone.to_string(),
            assistant_id: input.assistant_id,
            message: input.message,
            yolo_mode: input.yolo_mode,
            workspace_override: input.workspace_override,
            next_run_at: Some(next_run_at),
        })
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn list_scheduled_tasks(
        repo: &dyn ScheduledTaskRepository,
        assistant_id: Option<&str>,
    ) -> Result<Vec<ScheduledTaskModel>, String> {
        repo.list_scheduled_tasks(assistant_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
    ) -> Result<Option<ScheduledTaskModel>, String> {
        repo.get_scheduled_task(id).await.map_err(|e| e.to_string())
    }

    pub async fn update_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
        mut params: UpdateScheduledTaskParams,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let existing = repo
            .get_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("ScheduledTask {id} not found"))?;

        let effective_schedule_timezone = params
            .schedule_timezone
            .as_deref()
            .unwrap_or(existing.schedule_timezone.as_str());
        let normalized_schedule_timezone =
            normalize_schedule_timezone(effective_schedule_timezone)?;
        let should_recompute_next_run = params.cron_expression.is_some()
            || params.schedule_timezone.is_some()
            || (matches!(params.enabled, Some(true)) && !existing.enabled);

        if should_recompute_next_run {
            let effective_cron_expression = params
                .cron_expression
                .as_deref()
                .unwrap_or(existing.cron_expression.as_str());
            let next_run_at = compute_next_run_for_schedule_timezone(
                effective_cron_expression,
                now_ms,
                normalized_schedule_timezone,
            )?
            .ok_or_else(|| {
                format!(
                    "Invalid cron expression '{}': no future occurrences found",
                    effective_cron_expression
                )
            })?;
            params.next_run_at = Some(Some(next_run_at));
        }

        if params.schedule_timezone.is_some() {
            params.schedule_timezone = Some(normalized_schedule_timezone.to_string());
        }

        repo.update_scheduled_task(id, params)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn toggle_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
        enabled: bool,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let existing = repo
            .get_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("ScheduledTask {id} not found"))?;
        let next_run_at = if enabled {
            let schedule_timezone = normalize_schedule_timezone(&existing.schedule_timezone)?;
            Some(
                compute_next_run_for_schedule_timezone(
                    &existing.cron_expression,
                    now_ms,
                    schedule_timezone,
                )?
                .ok_or_else(|| {
                    format!(
                        "Invalid cron expression '{}': no future occurrences found",
                        existing.cron_expression
                    )
                })?,
            )
        } else {
            existing.next_run_at
        };

        repo.update_scheduled_task(
            id,
            UpdateScheduledTaskParams {
                name: None,
                cron_expression: None,
                schedule_timezone: None,
                assistant_id: None,
                message: None,
                yolo_mode: None,
                workspace_override: None,
                enabled: Some(enabled),
                next_run_at: Some(next_run_at),
            },
        )
        .await
        .map_err(|e| e.to_string())
    }
    pub async fn delete_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
    ) -> Result<(), String> {
        repo.delete_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())
    }
}

fn normalize_schedule_timezone(schedule_timezone: &str) -> Result<&'static str, String> {
    Ok(match ScheduleTimezone::parse(schedule_timezone)? {
        ScheduleTimezone::Utc => ScheduleTimezone::Utc.as_str(),
        ScheduleTimezone::Local => ScheduleTimezone::Local.as_str(),
    })
}

pub fn default_schedule_timezone() -> &'static str {
    SCHEDULE_TIMEZONE_LOCAL
}

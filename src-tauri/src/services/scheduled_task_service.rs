use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::{
    CreateScheduledTaskParams, ScheduledTaskRepository, UpdateScheduledTaskParams,
};
use crate::scheduled::runner::compute_next_run;
use uuid::Uuid;

pub struct ScheduledTaskService;

impl ScheduledTaskService {
    pub async fn create_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        name: String,
        cron_expression: String,
        assistant_id: String,
        message: String,
        yolo_mode: bool,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let next_run_at = compute_next_run(&cron_expression, now_ms).ok_or_else(|| {
            format!(
                "Invalid cron expression '{}': no future occurrences found",
                cron_expression
            )
        })?;
        repo.create_scheduled_task(CreateScheduledTaskParams {
            id: Uuid::new_v4().to_string(),
            name,
            cron_expression,
            assistant_id,
            message,
            yolo_mode,
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
        name: Option<String>,
        cron_expression: Option<String>,
        message: Option<String>,
        yolo_mode: Option<bool>,
        enabled: Option<bool>,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let next_run_at: Option<Option<i64>> = cron_expression
            .as_deref()
            .map(|expr| {
                compute_next_run(expr, now_ms).ok_or_else(|| {
                    format!("Invalid cron expression '{expr}': no future occurrences found")
                })
            })
            .transpose()?
            .map(Some);

        repo.update_scheduled_task(
            id,
            UpdateScheduledTaskParams {
                name,
                cron_expression,
                message,
                yolo_mode,
                enabled,
                next_run_at,
            },
        )
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn toggle_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
        enabled: bool,
    ) -> Result<ScheduledTaskModel, String> {
        repo.update_scheduled_task(
            id,
            UpdateScheduledTaskParams {
                name: None,
                cron_expression: None,
                message: None,
                yolo_mode: None,
                enabled: Some(enabled),
                next_run_at: None,
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

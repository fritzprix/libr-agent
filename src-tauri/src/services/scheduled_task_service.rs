use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::ScheduledTaskRepository;
use crate::scheduled::runner::compute_next_run;
use crate::state::get_scheduled_task_repository;
use uuid::Uuid;

pub struct ScheduledTaskService;

impl ScheduledTaskService {
    pub async fn create_scheduled_task(
        name: String,
        cron_expression: String,
        assistant_id: String,
        message: String,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let next_run_at = compute_next_run(&cron_expression, now_ms).ok_or_else(|| {
            format!(
                "Invalid cron expression '{}': no future occurrences found",
                cron_expression
            )
        })?;
        get_scheduled_task_repository()
            .create_scheduled_task(
                Uuid::new_v4().to_string(),
                name,
                cron_expression,
                assistant_id,
                message,
                Some(next_run_at),
            )
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn list_scheduled_tasks(
        assistant_id: Option<&str>,
    ) -> Result<Vec<ScheduledTaskModel>, String> {
        get_scheduled_task_repository()
            .list_scheduled_tasks(assistant_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_scheduled_task(id: &str) -> Result<Option<ScheduledTaskModel>, String> {
        get_scheduled_task_repository()
            .get_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn update_scheduled_task(
        id: &str,
        name: Option<String>,
        cron_expression: Option<String>,
        message: Option<String>,
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

        get_scheduled_task_repository()
            .update_scheduled_task(
                id,
                name,
                cron_expression,
                message,
                enabled,
                next_run_at,
            )
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn toggle_scheduled_task(id: &str, enabled: bool) -> Result<ScheduledTaskModel, String> {
        get_scheduled_task_repository()
            .update_scheduled_task(id, None, None, None, Some(enabled), None)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn delete_scheduled_task(id: &str) -> Result<(), String> {
        get_scheduled_task_repository()
            .delete_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())
    }
}

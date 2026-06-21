use crate::agent::ExecutionMode;
use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{
    CreateScheduledTaskParams, ScheduledTaskRepository, UpdateScheduledTaskParams,
};
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::scheduled::{
    is_one_shot_task, is_session_task, normalize_cron, ScheduleTimezone, SCHEDULE_TIMEZONE_LOCAL,
    TASK_CATEGORY_GLOBAL,
};
use crate::state::get_settings_repository;
use chrono::TimeZone;
use cron::Schedule;
use std::str::FromStr;
use uuid::Uuid;

pub struct ScheduledTaskService;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScheduledTaskGovernanceSettings {
    pub minimum_interval_minutes: u64,
}

pub struct CreateScheduledTaskInput {
    pub name: String,
    pub task_category: String,
    pub cron_expression: Option<String>,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub message: String,
    pub execution_mode: ExecutionMode,
    pub created_by_session_id: Option<String>,
    pub session_id: Option<String>,
    pub workspace_override: Option<String>,
    pub next_run_at: Option<i64>,
}

impl ScheduledTaskService {
    pub async fn create_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        input: CreateScheduledTaskInput,
    ) -> Result<ScheduledTaskModel, String> {
        let governance = load_governance_settings().await;
        Self::create_scheduled_task_with_governance(repo, input, &governance).await
    }

    pub async fn create_scheduled_task_with_governance(
        repo: &dyn ScheduledTaskRepository,
        input: CreateScheduledTaskInput,
        governance: &ScheduledTaskGovernanceSettings,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let normalized_schedule_timezone = normalize_schedule_timezone(&input.schedule_timezone)?;
        let is_session = is_session_task(&input.task_category);

        if is_session {
            if input.session_id.as_deref().is_none_or(str::is_empty) {
                return Err("SESSION tasks require a session_id".to_string());
            }
        } else if input.task_category != TASK_CATEGORY_GLOBAL {
            return Err(format!("Unknown task_category '{}'", input.task_category));
        } else if input.cron_expression.as_deref().is_none_or(str::is_empty) {
            return Err("GLOBAL tasks require a cron_expression".to_string());
        }

        let is_one_shot = is_session && is_one_shot_task(&input.cron_expression);
        if !is_one_shot {
            if let Some(cron_expression) = input.cron_expression.as_deref() {
                enforce_minimum_interval(
                    cron_expression,
                    normalized_schedule_timezone,
                    governance,
                )?;
            } else {
                return Err("Recurring tasks require a cron_expression".to_string());
            }
        }

        let next_run_at = if let Some(precomputed) = input.next_run_at {
            precomputed
        } else if let Some(cron_expression) = input.cron_expression.as_deref() {
            compute_next_run_for_schedule_timezone(
                cron_expression,
                now_ms,
                normalized_schedule_timezone,
            )?
            .ok_or_else(|| {
                format!(
                    "Invalid cron expression '{}': no future occurrences found",
                    cron_expression
                )
            })?
        } else {
            return Err("Either next_run_at or cron_expression is required".to_string());
        };

        repo.create_scheduled_task(CreateScheduledTaskParams {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            task_category: input.task_category,
            cron_expression: input.cron_expression,
            schedule_timezone: normalized_schedule_timezone.to_string(),
            assistant_id: input.assistant_id,
            message: input.message,
            execution_mode: input.execution_mode,
            created_by_session_id: input.created_by_session_id,
            session_id: input.session_id,
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
        params: UpdateScheduledTaskParams,
    ) -> Result<ScheduledTaskModel, String> {
        let governance = load_governance_settings().await;
        Self::update_scheduled_task_with_governance(repo, id, params, &governance).await
    }

    pub async fn update_scheduled_task_with_governance(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
        mut params: UpdateScheduledTaskParams,
        governance: &ScheduledTaskGovernanceSettings,
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
                .or(existing.cron_expression.as_deref())
                .ok_or_else(|| {
                    "Cannot recompute next run for a task without a cron expression".to_string()
                })?;
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
            enforce_minimum_interval(
                effective_cron_expression,
                normalized_schedule_timezone,
                governance,
            )?;
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
        let governance = load_governance_settings().await;
        Self::toggle_scheduled_task_with_governance(repo, id, enabled, &governance).await
    }

    pub async fn toggle_scheduled_task_with_governance(
        repo: &dyn ScheduledTaskRepository,
        id: &str,
        enabled: bool,
        governance: &ScheduledTaskGovernanceSettings,
    ) -> Result<ScheduledTaskModel, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let existing = repo
            .get_scheduled_task(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("ScheduledTask {id} not found"))?;
        let next_run_at = if enabled {
            let schedule_timezone = normalize_schedule_timezone(&existing.schedule_timezone)?;
            let cron_expression = existing.cron_expression.as_deref().ok_or_else(|| {
                "Cannot re-enable a one-shot session callback without a cron expression".to_string()
            })?;
            enforce_minimum_interval(cron_expression, schedule_timezone, governance)?;
            Some(
                compute_next_run_for_schedule_timezone(cron_expression, now_ms, schedule_timezone)?
                    .ok_or_else(|| {
                        format!(
                            "Invalid cron expression '{}': no future occurrences found",
                            cron_expression
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
                execution_mode: None,
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

    pub async fn list_session_scheduled_tasks(
        repo: &dyn ScheduledTaskRepository,
        session_id: &str,
    ) -> Result<Vec<ScheduledTaskModel>, String> {
        repo.list_session_scheduled_tasks(session_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn cancel_session_scheduled_task(
        repo: &dyn ScheduledTaskRepository,
        session_id: &str,
        task_id: &str,
    ) -> Result<(), String> {
        let task = repo
            .get_scheduled_task(task_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("ScheduledTask {task_id} not found"))?;

        if !is_session_task(&task.task_category) {
            return Err(
                "Only session callbacks can be cancelled from the session panel".to_string(),
            );
        }

        if task.session_id.as_deref() != Some(session_id) {
            return Err("Session callback does not belong to this session".to_string());
        }

        repo.delete_scheduled_task(task_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn delete_session_scheduled_tasks_for_sessions(
        repo: &dyn ScheduledTaskRepository,
        session_ids: &[String],
    ) -> Result<u64, String> {
        if session_ids.is_empty() {
            return Ok(0);
        }

        let ids: Vec<&str> = session_ids.iter().map(String::as_str).collect();
        repo.delete_session_scheduled_tasks(&ids)
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

async fn load_governance_settings() -> ScheduledTaskGovernanceSettings {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SystemSettings {
        scheduled_task_minimum_interval_minutes: Option<u64>,
    }

    let Ok(Some(model)) = get_settings_repository().get("systemSettings").await else {
        return ScheduledTaskGovernanceSettings::default();
    };

    let Ok(settings) = serde_json::from_str::<SystemSettings>(&model.value) else {
        return ScheduledTaskGovernanceSettings::default();
    };

    ScheduledTaskGovernanceSettings {
        minimum_interval_minutes: settings
            .scheduled_task_minimum_interval_minutes
            .unwrap_or(0),
    }
}

fn enforce_minimum_interval(
    cron_expression: &str,
    schedule_timezone: &str,
    governance: &ScheduledTaskGovernanceSettings,
) -> Result<(), String> {
    if governance.minimum_interval_minutes == 0 {
        return Ok(());
    }

    let Some(interval_ms) = compute_schedule_interval_ms(
        cron_expression,
        schedule_timezone,
        chrono::Utc::now().timestamp_millis(),
    )?
    else {
        return Ok(());
    };

    let minimum_interval_ms = (governance.minimum_interval_minutes as i64) * 60 * 1000;
    if interval_ms < minimum_interval_ms {
        return Err(format!(
            "Scheduled task interval is too frequent. Minimum allowed interval is {} minute(s).",
            governance.minimum_interval_minutes
        ));
    }

    Ok(())
}

fn compute_schedule_interval_ms(
    cron_expression: &str,
    schedule_timezone: &str,
    reference_ms: i64,
) -> Result<Option<i64>, String> {
    let normalized = normalize_cron(cron_expression);
    let schedule = Schedule::from_str(&normalized)
        .map_err(|error| format!("Invalid cron expression '{}': {}", cron_expression, error))?;

    let timezone = ScheduleTimezone::parse(schedule_timezone)?;
    let interval = match timezone {
        ScheduleTimezone::Utc => interval_after(&schedule, chrono::Utc, reference_ms),
        ScheduleTimezone::Local => interval_after(&schedule, chrono::Local, reference_ms),
    };

    Ok(interval)
}

fn interval_after<Tz: TimeZone>(
    schedule: &Schedule,
    timezone: Tz,
    reference_ms: i64,
) -> Option<i64> {
    let after = timezone
        .timestamp_millis_opt(reference_ms + 1000)
        .single()?;
    let mut occurrences = schedule.after(&after);
    let first = occurrences.next()?;
    let second = occurrences.next()?;
    Some(second.timestamp_millis() - first.timestamp_millis())
}

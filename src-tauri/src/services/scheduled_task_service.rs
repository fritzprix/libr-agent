use crate::entity::scheduled_task::Model as ScheduledTaskModel;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{
    CreateScheduledTaskParams, ScheduledTaskRepository, UpdateScheduledTaskParams,
};
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::scheduled::{normalize_cron, ScheduleTimezone, SCHEDULE_TIMEZONE_LOCAL};
use crate::state::get_settings_repository;
use chrono::TimeZone;
use cron::Schedule;
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

pub struct ScheduledTaskService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTaskGovernanceSettings {
    pub minimum_interval_minutes: u64,
    pub max_scheduled_task_groups: usize,
}

impl Default for ScheduledTaskGovernanceSettings {
    fn default() -> Self {
        Self {
            minimum_interval_minutes: 0,
            max_scheduled_task_groups: 10,
        }
    }
}

pub struct CreateScheduledTaskInput {
    pub name: String,
    pub cron_expression: String,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub message: String,
    pub yolo_mode: bool,
    pub created_by_session_id: Option<String>,
    pub workspace_override: Option<String>,
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
        enforce_minimum_interval(
            &input.cron_expression,
            normalized_schedule_timezone,
            governance,
        )?;
        let (group_id, group_name) =
            normalize_group_fields_with_repo(repo, input.group_id, input.group_name).await?;
        enforce_group_limit(repo, group_id.as_deref(), None, governance).await?;
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
            group_id,
            group_name,
            message: input.message,
            yolo_mode: input.yolo_mode,
            created_by_session_id: input.created_by_session_id,
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
        let (effective_group_id, effective_group_name) = resolve_updated_group_fields(
            existing.group_id.as_deref(),
            params.group_id.take(),
            existing.group_name.as_deref(),
            params.group_name.take(),
        )?;
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
            enforce_minimum_interval(
                effective_cron_expression,
                normalized_schedule_timezone,
                governance,
            )?;
        }

        if params.schedule_timezone.is_some() {
            params.schedule_timezone = Some(normalized_schedule_timezone.to_string());
        }
        params.group_id = Some(effective_group_id.clone());
        params.group_name = Some(effective_group_name.clone());

        // Only enforce group cap when the group is actually changing. If the
        // effective group ID is the same as the existing one we are not creating a
        // new group, so filtering out the current task would produce a false
        // "cap exceeded" error when the task is the sole member of that group.
        let group_has_changed = effective_group_id.as_deref() != existing.group_id.as_deref();
        if group_has_changed {
            enforce_group_limit(repo, effective_group_id.as_deref(), Some(id), governance).await?;
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
            enforce_minimum_interval(&existing.cron_expression, schedule_timezone, governance)?;
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
                group_id: None,
                group_name: None,
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

async fn load_governance_settings() -> ScheduledTaskGovernanceSettings {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SystemSettings {
        scheduled_task_minimum_interval_minutes: Option<u64>,
        max_scheduled_task_groups: Option<u64>,
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
        max_scheduled_task_groups: settings.max_scheduled_task_groups.unwrap_or(10) as usize,
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

async fn enforce_group_limit(
    repo: &dyn ScheduledTaskRepository,
    effective_group_id: Option<&str>,
    current_task_id: Option<&str>,
    governance: &ScheduledTaskGovernanceSettings,
) -> Result<(), String> {
    let Some(group_id) = effective_group_id else {
        return Ok(());
    };

    let tasks = repo
        .list_scheduled_tasks(None)
        .await
        .map_err(|e| e.to_string())?;

    let existing_groups = tasks
        .into_iter()
        .filter(|task| Some(task.id.as_str()) != current_task_id)
        .filter_map(|task| task.group_id)
        .collect::<HashSet<_>>();

    if !existing_groups.contains(group_id)
        && existing_groups.len() >= governance.max_scheduled_task_groups
    {
        return Err(format!(
            "Maximum scheduled task groups reached (limit: {}). Reuse an existing group or increase the limit in Settings.",
            governance.max_scheduled_task_groups
        ));
    }

    Ok(())
}

fn normalize_group_fields(
    group_id: Option<String>,
    group_name: Option<String>,
) -> Result<(Option<String>, Option<String>), String> {
    let normalized_group_name = group_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    match (group_id, normalized_group_name) {
        (None, None) => Ok((None, None)),
        // groupId-only is handled by the async variant; this sync path
        // requires groupName to be present when provided.
        (Some(_group_id), None) => Err(
            "groupId requires groupName (or use groupId-only join via createScheduledTask)"
                .to_string(),
        ),
        (group_id, Some(group_name)) => {
            let normalized_group_id = group_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| slugify_group_name(&group_name));
            Ok((Some(normalized_group_id), Some(group_name)))
        }
    }
}

/// Async variant of normalize_group_fields that resolves groupName from the
/// repository when only groupId is provided, matching the schedule contract:
/// "first task supplies groupName to create; subsequent tasks join with groupId only."
async fn normalize_group_fields_with_repo(
    repo: &dyn ScheduledTaskRepository,
    group_id: Option<String>,
    group_name: Option<String>,
) -> Result<(Option<String>, Option<String>), String> {
    let normalized_group_name = group_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    match (group_id, normalized_group_name) {
        (None, None) => Ok((None, None)),
        // groupName only → auto-generate groupId from the name.
        (None, Some(group_name)) => {
            let group_id = slugify_group_name(&group_name);
            Ok((Some(group_id), Some(group_name)))
        }
        // Both provided → normalize normally.
        (Some(group_id), Some(group_name)) => {
            let normalized_id = group_id.trim().to_string();
            if normalized_id.is_empty() {
                return Err("groupId cannot be an empty string".to_string());
            }
            Ok((Some(normalized_id), Some(group_name)))
        }
        // groupId only → resolve groupName from an existing task in that group.
        (Some(group_id), None) => {
            let normalized_id = group_id.trim().to_string();
            if normalized_id.is_empty() {
                return Err("groupId cannot be an empty string".to_string());
            }
            let tasks = repo
                .list_scheduled_tasks(None)
                .await
                .map_err(|e| e.to_string())?;
            let resolved_name = tasks
                .into_iter()
                .find(|task| task.group_id.as_deref() == Some(normalized_id.as_str()))
                .and_then(|task| task.group_name)
                .ok_or_else(|| format!(
                    "groupId '{}' does not match any existing group. Provide groupName to create a new group.",
                    normalized_id
                ))?;
            Ok((Some(normalized_id), Some(resolved_name)))
        }
    }
}

fn resolve_updated_group_fields(
    existing_group_id: Option<&str>,
    group_id_update: Option<Option<String>>,
    existing_group_name: Option<&str>,
    group_name_update: Option<Option<String>>,
) -> Result<(Option<String>, Option<String>), String> {
    match (group_id_update, group_name_update) {
        (None, None) => Ok((
            existing_group_id.map(ToString::to_string),
            existing_group_name.map(ToString::to_string),
        )),
        (Some(None), Some(None)) => Ok((None, None)),
        (Some(None), None) | (None, Some(None)) => Ok((None, None)),
        (Some(Some(group_id)), Some(Some(group_name))) => {
            normalize_group_fields(Some(group_id), Some(group_name))
        }
        (None, Some(Some(group_name))) => {
            let inherited_group_id = existing_group_id.map(ToString::to_string);
            normalize_group_fields(inherited_group_id, Some(group_name))
        }
        (Some(Some(group_id)), None) => Err(format!(
            "groupId '{}' requires groupName when updating scheduled task group",
            group_id
        )),
        (Some(None), Some(Some(_))) | (Some(Some(_)), Some(None)) => {
            Err("clearGroup cannot be combined with groupId/groupName values".to_string())
        }
    }
}

fn slugify_group_name(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;

    for ch in value.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            result.push('-');
            previous_dash = true;
        }
    }

    let trimmed = result.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "scheduled-group".to_string()
    } else {
        trimmed
    }
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

use super::formatting::{format_timestamp, render_task_detail, render_task_line, task_to_json};
use super::ScheduledTaskServer;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, not_found_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::repositories::{AssistantRepository, UpdateScheduledTaskParams};
use crate::services::{default_schedule_timezone, CreateScheduledTaskInput, ScheduledTaskService};
use crate::state::{get_assistant_repository, get_scheduled_task_repository};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduledTaskArgs {
    name: String,
    cron_expression: String,
    schedule_timezone: Option<String>,
    assistant_id: String,
    group_id: Option<String>,
    group_name: Option<String>,
    message: String,
    yolo_mode: Option<bool>,
    workspace_override: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListScheduledTasksArgs {
    assistant_id: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskIdArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScheduledTaskArgs {
    id: String,
    name: Option<String>,
    cron_expression: Option<String>,
    schedule_timezone: Option<String>,
    assistant_id: Option<String>,
    group_id: Option<String>,
    group_name: Option<String>,
    message: Option<String>,
    yolo_mode: Option<bool>,
    workspace_override: Option<String>,
    clear_workspace_override: Option<bool>,
    clear_group: Option<bool>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleScheduledTaskArgs {
    id: String,
    enabled: bool,
}

pub async fn handle_create_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
    session_id: Option<String>,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: CreateScheduledTaskArgs = match parse_args(args, "createScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    if let Err(result) = validate_assistant_id(&args.assistant_id).await {
        return Ok(result);
    }
    if let Some(workspace_override) = args.workspace_override.as_deref() {
        if let Err(result) = validate_workspace_override(workspace_override).await {
            return Ok(result);
        }
    }

    let created = match ScheduledTaskService::create_scheduled_task(
        get_scheduled_task_repository(),
        CreateScheduledTaskInput {
            name: args.name,
            cron_expression: args.cron_expression,
            schedule_timezone: args
                .schedule_timezone
                .unwrap_or_else(|| default_schedule_timezone().to_string()),
            assistant_id: args.assistant_id,
            group_id: args.group_id,
            group_name: args.group_name,
            message: args.message,
            yolo_mode: args.yolo_mode.unwrap_or(false),
            created_by_session_id: session_id,
            workspace_override: args.workspace_override,
        },
    )
    .await
    {
        Ok(task) => task,
        Err(error) => return Ok(service_error_result("Create Scheduled Task", &error)),
    };

    let text = format!(
        "Scheduled task created (ID: {}).\n\n{}\n\n💡 Next steps:\n1. Use getScheduledTask(\"{}\") to inspect the full task.\n2. Use toggleScheduledTask(\"{}\", enabled=false) to pause it.\n3. Use updateScheduledTask(\"{}\", ...) to revise schedule or message.",
        created.id,
        render_task_detail(&created),
        created.id,
        created.id,
        created.id
    );

    Ok(SuccessHint::new(
        text,
        vec![format!(
            "Use getScheduledTask(\"{}\") to inspect the full task before creating related schedules",
            created.id
        )],
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&created)
    }))))
}

pub async fn handle_list_scheduled_tasks(
    _server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: ListScheduledTasksArgs = match parse_args(args, "listScheduledTasks") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    if let Some(assistant_id) = args.assistant_id.as_deref() {
        if let Err(result) = validate_assistant_id(assistant_id).await {
            return Ok(result);
        }
    }

    let mut tasks = match ScheduledTaskService::list_scheduled_tasks(
        get_scheduled_task_repository(),
        args.assistant_id.as_deref(),
    )
    .await
    {
        Ok(tasks) => tasks,
        Err(error) => return Ok(service_error_result("List Scheduled Tasks", &error)),
    };

    if let Some(enabled) = args.enabled {
        tasks.retain(|task| task.enabled == enabled);
    }

    let header = match (args.assistant_id.as_deref(), args.enabled) {
        (Some(assistant_id), Some(enabled)) => format!(
            "Found {} scheduled task(s) for assistant '{}' with enabled={}:",
            tasks.len(),
            assistant_id,
            enabled
        ),
        (Some(assistant_id), None) => format!(
            "Found {} scheduled task(s) for assistant '{}':",
            tasks.len(),
            assistant_id
        ),
        (None, Some(enabled)) => format!(
            "Found {} scheduled task(s) with enabled={}:",
            tasks.len(),
            enabled
        ),
        (None, None) => format!("Found {} scheduled task(s):", tasks.len()),
    };

    let body = if tasks.is_empty() {
        "No scheduled tasks matched the filter.".to_string()
    } else {
        tasks
            .iter()
            .map(render_task_line)
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(SuccessHint::new(
        format!(
            "{header}\n\n{body}\n\n💡 Use getScheduledTask(\"...\") for full detail before updating or deleting."
        ),
        vec!["Use getScheduledTask(\"...\") to inspect one task in detail".to_string()],
    )
    .to_mcp_result_with_data(Some(json!({
        "tasks": tasks.iter().map(task_to_json).collect::<Vec<_>>(),
        "total": tasks.len()
    }))))
}

pub async fn handle_get_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: ScheduledTaskIdArgs = match parse_args(args, "getScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let task =
        match ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &args.id)
            .await
        {
            Ok(Some(task)) => task,
            Ok(None) => {
                return Ok(not_found_error(
                    "Scheduled task",
                    &args.id,
                    ToolGroup::ScheduledTask,
                ))
            }
            Err(error) => return Ok(service_error_result("Get Scheduled Task", &error)),
        };

    Ok(SuccessHint::new(
        format!(
            "{}\n\n💡 Use updateScheduledTask(\"{}\", ...) to modify or toggleScheduledTask(\"{}\", enabled=false) to pause.",
            render_task_detail(&task),
            task.id,
            task.id
        ),
        vec![format!(
            "Use updateScheduledTask(\"{}\", ...) to change schedule or message",
            task.id
        )],
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&task)
    }))))
}

pub async fn handle_update_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: UpdateScheduledTaskArgs = match parse_args(args, "updateScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    if args.clear_workspace_override.unwrap_or(false) && args.workspace_override.is_some() {
        return Ok(invalid_input_error(
            "workspaceOverride and clearWorkspaceOverride=true cannot be used together",
            ToolGroup::ScheduledTask,
        ));
    }
    if args.clear_group.unwrap_or(false) && (args.group_id.is_some() || args.group_name.is_some()) {
        return Ok(invalid_input_error(
            "groupId/groupName and clearGroup=true cannot be used together",
            ToolGroup::ScheduledTask,
        ));
    }

    if let Some(assistant_id) = args.assistant_id.as_deref() {
        if let Err(result) = validate_assistant_id(assistant_id).await {
            return Ok(result);
        }
    }
    if let Some(workspace_override) = args.workspace_override.as_deref() {
        if let Err(result) = validate_workspace_override(workspace_override).await {
            return Ok(result);
        }
    }

    let workspace_override = if args.clear_workspace_override.unwrap_or(false) {
        Some(None)
    } else {
        args.workspace_override.map(Some)
    };

    let mut changed_fields = Vec::new();
    collect_changed_field(&mut changed_fields, "name", args.name.is_some());
    collect_changed_field(
        &mut changed_fields,
        "cronExpression",
        args.cron_expression.is_some(),
    );
    collect_changed_field(
        &mut changed_fields,
        "scheduleTimezone",
        args.schedule_timezone.is_some(),
    );
    collect_changed_field(
        &mut changed_fields,
        "assistantId",
        args.assistant_id.is_some(),
    );
    collect_changed_field(
        &mut changed_fields,
        "group",
        args.group_id.is_some() || args.group_name.is_some() || args.clear_group.is_some(),
    );
    collect_changed_field(&mut changed_fields, "message", args.message.is_some());
    collect_changed_field(&mut changed_fields, "yoloMode", args.yolo_mode.is_some());
    collect_changed_field(
        &mut changed_fields,
        "workspaceOverride",
        workspace_override.is_some(),
    );
    collect_changed_field(&mut changed_fields, "enabled", args.enabled.is_some());

    if changed_fields.is_empty() {
        return Ok(invalid_input_error(
            "Provide at least one mutable field to update",
            ToolGroup::ScheduledTask,
        ));
    }

    let task_id = args.id.clone();
    let updated = match ScheduledTaskService::update_scheduled_task(
        get_scheduled_task_repository(),
        &task_id,
        UpdateScheduledTaskParams {
            name: args.name,
            cron_expression: args.cron_expression,
            schedule_timezone: args.schedule_timezone,
            assistant_id: args.assistant_id,
            group_id: if args.clear_group.unwrap_or(false) {
                Some(None)
            } else {
                args.group_id.map(Some)
            },
            group_name: if args.clear_group.unwrap_or(false) {
                Some(None)
            } else {
                args.group_name.map(Some)
            },
            message: args.message,
            yolo_mode: args.yolo_mode,
            workspace_override,
            enabled: args.enabled,
            next_run_at: None,
        },
    )
    .await
    {
        Ok(task) => task,
        Err(error) if error.contains("not found") => {
            return Ok(not_found_error(
                "Scheduled task",
                &task_id,
                ToolGroup::ScheduledTask,
            ));
        }
        Err(error) => return Ok(service_error_result("Update Scheduled Task", &error)),
    };

    let changed_summary = changed_fields.join(", ");
    Ok(SuccessHint::new(
        format!(
            "Scheduled task updated (ID: {}).\n\nChanged fields: {}\nNext run: {}\nEnabled: {}\n\n💡 Use getScheduledTask(\"{}\") to confirm the persisted state.",
            updated.id,
            changed_summary,
            format_timestamp(updated.next_run_at),
            updated.enabled,
            updated.id
        ),
        vec![format!(
            "Use getScheduledTask(\"{}\") to confirm the persisted schedule",
            updated.id
        )],
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&updated),
        "changedFields": changed_fields
    }))))
}

pub async fn handle_toggle_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: ToggleScheduledTaskArgs = match parse_args(args, "toggleScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let task_id = args.id.clone();
    let updated = match ScheduledTaskService::toggle_scheduled_task(
        get_scheduled_task_repository(),
        &task_id,
        args.enabled,
    )
    .await
    {
        Ok(task) => task,
        Err(error) if error.contains("not found") => {
            return Ok(not_found_error(
                "Scheduled task",
                &task_id,
                ToolGroup::ScheduledTask,
            ));
        }
        Err(error) => return Ok(service_error_result("Toggle Scheduled Task", &error)),
    };

    let state_text = if updated.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(SuccessHint::new(
        format!(
            "Scheduled task {} (ID: {}) is now {}.\nNext run: {}\n\n💡 Use getScheduledTask(\"{}\") to inspect the full schedule state.",
            updated.name,
            updated.id,
            state_text,
            format_timestamp(updated.next_run_at),
            updated.id
        ),
        vec![format!(
            "Use getScheduledTask(\"{}\") to inspect the full task state",
            updated.id
        )],
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&updated)
    }))))
}

pub async fn handle_delete_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: ScheduledTaskIdArgs = match parse_args(args, "deleteScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let existing =
        match ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &args.id)
            .await
        {
            Ok(Some(task)) => task,
            Ok(None) => {
                return Ok(not_found_error(
                    "Scheduled task",
                    &args.id,
                    ToolGroup::ScheduledTask,
                ))
            }
            Err(error) => return Ok(service_error_result("Delete Scheduled Task", &error)),
        };

    if let Err(error) =
        ScheduledTaskService::delete_scheduled_task(get_scheduled_task_repository(), &args.id).await
    {
        return Ok(service_error_result("Delete Scheduled Task", &error));
    }

    Ok(SuccessHint::new(
        format!(
            "Scheduled task deleted (ID: {}).\n\nDeleted task summary:\n{}\n\n💡 Use listScheduledTasks() to verify the remaining schedule set.",
            existing.id,
            render_task_line(&existing)
        ),
        vec!["Use listScheduledTasks() to verify the remaining schedule set".to_string()],
    )
    .to_mcp_result_with_data(Some(json!({
        "deletedTask": task_to_json(&existing)
    }))))
}

fn collect_changed_field(changed_fields: &mut Vec<String>, field: &str, changed: bool) {
    if changed {
        changed_fields.push(field.to_string());
    }
}

fn parse_args<T>(args: Value, tool_name: &str) -> Result<T, crate::mcp::types::MCPResult>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args).map_err(|error| {
        invalid_input_error(
            &format!("Invalid {tool_name} arguments: {error}"),
            ToolGroup::ScheduledTask,
        )
    })
}

async fn validate_assistant_id(assistant_id: &str) -> Result<(), crate::mcp::types::MCPResult> {
    let exists = get_assistant_repository()
        .get_assistant(assistant_id)
        .await
        .map_err(|error| {
            service_error_result(
                "Validate Assistant",
                &format!("Failed to validate assistant '{}': {}", assistant_id, error),
            )
        })?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(invalid_input_error(
            &format!("Assistant '{}' not found", assistant_id),
            ToolGroup::ScheduledTask,
        ))
    }
}

async fn validate_workspace_override(path_str: &str) -> Result<(), crate::mcp::types::MCPResult> {
    let path = std::path::PathBuf::from(path_str);
    if !path.is_absolute() {
        return Err(invalid_input_error(
            "Workspace override must be an absolute path",
            ToolGroup::ScheduledTask,
        ));
    }
    if crate::services::agent_service::is_restricted_system_path(&path) {
        return Err(invalid_input_error(
            &format!(
                "Workspace override '{}' points to a restricted system directory",
                path_str
            ),
            ToolGroup::ScheduledTask,
        ));
    }

    let metadata = tokio::fs::metadata(&path).await.map_err(|error| {
        invalid_input_error(
            &format!("Workspace override is not accessible: {}", error),
            ToolGroup::ScheduledTask,
        )
    })?;
    if !metadata.is_dir() {
        return Err(invalid_input_error(
            "Workspace override must point to a directory",
            ToolGroup::ScheduledTask,
        ));
    }

    let _ = tokio::fs::read_dir(&path).await.map_err(|error| {
        invalid_input_error(
            &format!("Workspace override is not readable: {}", error),
            ToolGroup::ScheduledTask,
        )
    })?;

    Ok(())
}

fn service_error_result(operation: &str, error: &str) -> crate::mcp::types::MCPResult {
    if error.contains("Invalid cron expression")
        || error.contains("schedule timezone")
        || error.contains("timezone")
    {
        invalid_input_error(error, ToolGroup::ScheduledTask)
    } else {
        operation_failed_error(
            operation,
            error,
            vec![
                "Use listScheduledTasks() to inspect the current schedule state".to_string(),
                "Verify assistantId, cronExpression, and workspaceOverride values".to_string(),
            ],
            ToolGroup::ScheduledTask,
        )
    }
}

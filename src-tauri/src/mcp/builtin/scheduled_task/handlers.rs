use super::formatting::{format_timestamp, render_task_detail, render_task_line, task_to_json};
use super::ScheduledTaskServer;
use crate::agent::ExecutionMode;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, not_found_error, operation_failed_error, permission_denied_error,
    SuccessHint, ToolGroup,
};
use crate::repositories::{AssistantRepository, SessionRepository, UpdateScheduledTaskParams};
use crate::scheduled::runner::compute_next_run_for_schedule_timezone;
use crate::scheduled::{TASK_CATEGORY_GLOBAL, TASK_CATEGORY_SESSION};
use crate::services::{default_schedule_timezone, CreateScheduledTaskInput, ScheduledTaskService};
use crate::state::{
    get_assistant_repository, get_scheduled_task_repository, get_session_repository,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduledTaskArgs {
    name: String,
    cron_expression: String,
    schedule_timezone: Option<String>,
    assistant_id: String,
    message: String,
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
    message: Option<String>,
    workspace_override: Option<String>,
    clear_workspace_override: Option<bool>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleScheduledTaskArgs {
    id: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleCallbackArgs {
    message: String,
    name: Option<String>,
    delay_seconds: Option<u64>,
    cron_expression: Option<String>,
}

pub async fn handle_create_scheduled_task(
    _server: &ScheduledTaskServer,
    args: Value,
    session_id: Option<String>,
) -> Result<crate::mcp::types::MCPResult, String> {
    let execution_mode = match parse_execution_mode_for_create(&args) {
        Ok(mode) => mode,
        Err(result) => return Ok(result),
    };

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
            task_category: crate::scheduled::TASK_CATEGORY_GLOBAL.to_string(),
            cron_expression: Some(args.cron_expression),
            schedule_timezone: args
                .schedule_timezone
                .unwrap_or_else(|| default_schedule_timezone().to_string()),
            assistant_id: args.assistant_id,
            message: args.message,
            execution_mode,
            created_by_session_id: session_id,
            session_id: None,
            workspace_override: args.workspace_override,
            next_run_at: None,
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
    server: &ScheduledTaskServer,
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

    // Filter out session tasks belonging to other sessions
    tasks.retain(|task| {
        if task.task_category == TASK_CATEGORY_SESSION {
            task.session_id.as_deref() == Some(server.session_id.as_str())
        } else {
            true
        }
    });

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
    server: &ScheduledTaskServer,
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

    if let Err(result) = check_session_ownership(
        &task.task_category,
        task.session_id.as_deref(),
        server.session_id.as_str(),
    ) {
        return Ok(result);
    }

    let (guidance, success_hints) = match task.task_category.as_str() {
        TASK_CATEGORY_GLOBAL => {
            let guidance = format!(
                "💡 Use updateScheduledTask(\"{}\", ...) to modify, or toggleScheduledTask(\"{}\", enabled=false) to pause.",
                task.id, task.id
            );
            let hints = vec![format!(
                "Use updateScheduledTask(\"{}\", ...) to change schedule or message",
                task.id
            )];
            (guidance, hints)
        }
        TASK_CATEGORY_SESSION => {
            let guidance = format!(
                "💡 Use toggleScheduledTask(\"{}\", enabled=false) to pause, or deleteScheduledTask(\"{}\") to cancel.",
                task.id, task.id
            );
            let hints = vec![format!(
                "Use toggleScheduledTask(\"{}\", enabled=false) to pause the session callback, or deleteScheduledTask(\"{}\") to cancel it",
                task.id, task.id
            )];
            (guidance, hints)
        }
        _ => {
            let guidance =
                "💡 Use getScheduledTask(\"...\") to inspect the task details.".to_string();
            let hints =
                vec!["Use getScheduledTask(\"...\") to inspect the task details".to_string()];
            (guidance, hints)
        }
    };

    Ok(SuccessHint::new(
        format!("{}\n\n{}", render_task_detail(&task), guidance),
        success_hints,
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&task)
    }))))
}

pub async fn handle_update_scheduled_task(
    server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let execution_mode_update = match parse_execution_mode_for_update(&args) {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let args: UpdateScheduledTaskArgs = match parse_args(args, "updateScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let task_id = args.id.clone();
    let existing =
        match ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &task_id)
            .await
        {
            Ok(Some(task)) => task,
            Ok(None) => {
                return Ok(not_found_error(
                    "Scheduled task",
                    &task_id,
                    ToolGroup::ScheduledTask,
                ))
            }
            Err(error) => return Ok(service_error_result("Update Scheduled Task", &error)),
        };

    if let Err(result) = check_session_ownership(
        &existing.task_category,
        existing.session_id.as_deref(),
        server.session_id.as_str(),
    ) {
        return Ok(result);
    }

    if args.clear_workspace_override.unwrap_or(false) && args.workspace_override.is_some() {
        return Ok(invalid_input_error(
            "workspaceOverride and clearWorkspaceOverride=true cannot be used together",
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
    collect_changed_field(&mut changed_fields, "message", args.message.is_some());
    if execution_mode_update.is_some() {
        changed_fields.push("executionMode".to_string());
    }
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

    let updated = match ScheduledTaskService::update_scheduled_task(
        get_scheduled_task_repository(),
        &task_id,
        UpdateScheduledTaskParams {
            name: args.name,
            cron_expression: args.cron_expression,
            schedule_timezone: args.schedule_timezone,
            assistant_id: args.assistant_id,
            message: args.message,
            execution_mode: execution_mode_update,
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
    server: &ScheduledTaskServer,
    args: Value,
) -> Result<crate::mcp::types::MCPResult, String> {
    let args: ToggleScheduledTaskArgs = match parse_args(args, "toggleScheduledTask") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let task_id = args.id.clone();
    let existing =
        match ScheduledTaskService::get_scheduled_task(get_scheduled_task_repository(), &task_id)
            .await
        {
            Ok(Some(task)) => task,
            Ok(None) => {
                return Ok(not_found_error(
                    "Scheduled task",
                    &task_id,
                    ToolGroup::ScheduledTask,
                ))
            }
            Err(error) => return Ok(service_error_result("Toggle Scheduled Task", &error)),
        };

    if let Err(result) = check_session_ownership(
        &existing.task_category,
        existing.session_id.as_deref(),
        server.session_id.as_str(),
    ) {
        return Ok(result);
    }

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

pub async fn handle_schedule_callback(
    server: &ScheduledTaskServer,
    args: Value,
    session_id: Option<String>,
) -> Result<crate::mcp::types::MCPResult, String> {
    let execution_mode = match parse_execution_mode_for_create(&args) {
        Ok(mode) => mode,
        Err(result) => return Ok(result),
    };

    let args: ScheduleCallbackArgs = match parse_args(args, "scheduleCallback") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };

    let session_id = session_id
        .or_else(|| Some(server.session_id.clone()))
        .ok_or_else(|| "scheduleCallback requires an active session context".to_string())?;

    let assistant_id = match resolve_assistant_id_for_session(&session_id).await {
        Ok(assistant_id) => assistant_id,
        Err(result) => return Ok(result),
    };

    let now_ms = chrono::Utc::now().timestamp_millis();
    let schedule_timezone = default_schedule_timezone().to_string();

    let (cron_expression, next_run_at) = match (args.delay_seconds, args.cron_expression) {
        (Some(delay), None) => (None, Some(now_ms + (delay as i64) * 1000)),
        (None, Some(cron)) => {
            let next_run_at =
                compute_next_run_for_schedule_timezone(&cron, now_ms, schedule_timezone.as_str())?
                    .ok_or_else(|| {
                        format!("Invalid cron expression '{}': no future occurrences", cron)
                    })?;
            (Some(cron), Some(next_run_at))
        }
        (Some(_), Some(_)) => {
            return Ok(invalid_input_error(
                "Provide exactly one of delaySeconds or cronExpression, not both",
                ToolGroup::ScheduledTask,
            ));
        }
        (None, None) => {
            return Ok(invalid_input_error(
                "Provide exactly one of delaySeconds or cronExpression",
                ToolGroup::ScheduledTask,
            ));
        }
    };

    let created = match ScheduledTaskService::create_scheduled_task(
        get_scheduled_task_repository(),
        CreateScheduledTaskInput {
            name: args
                .name
                .unwrap_or_else(|| "Scheduled Callback".to_string()),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression,
            schedule_timezone,
            assistant_id,
            message: args.message,
            execution_mode,
            created_by_session_id: Some(session_id.clone()),
            session_id: Some(session_id),
            workspace_override: None,
            next_run_at,
        },
    )
    .await
    {
        Ok(task) => task,
        Err(error) => return Ok(service_error_result("Schedule Callback", &error)),
    };

    Ok(SuccessHint::new(
        format!(
            "Session callback scheduled (ID: {}).\n\n{}\n\n💡 Use getScheduledTask(\"{}\") to inspect it or toggleScheduledTask(\"{}\", enabled=false) to cancel.",
            created.id,
            render_task_detail(&created),
            created.id,
            created.id
        ),
        vec![format!(
            "Use getScheduledTask(\"{}\") to inspect the callback before scheduling another",
            created.id
        )],
    )
    .to_mcp_result_with_data(Some(json!({
        "task": task_to_json(&created)
    }))))
}

pub async fn handle_delete_scheduled_task(
    server: &ScheduledTaskServer,
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

    if let Err(result) = check_session_ownership(
        &existing.task_category,
        existing.session_id.as_deref(),
        server.session_id.as_str(),
    ) {
        return Ok(result);
    }

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

fn parse_execution_mode_for_create(
    args: &Value,
) -> Result<ExecutionMode, crate::mcp::types::MCPResult> {
    match args.get("executionMode") {
        None => Ok(ExecutionMode::Normal),
        Some(value) => parse_execution_mode_value(value),
    }
}

fn parse_execution_mode_for_update(
    args: &Value,
) -> Result<Option<ExecutionMode>, crate::mcp::types::MCPResult> {
    match args.get("executionMode") {
        None => Ok(None),
        Some(value) => parse_execution_mode_value(value).map(Some),
    }
}

fn parse_execution_mode_value(
    value: &Value,
) -> Result<ExecutionMode, crate::mcp::types::MCPResult> {
    let mode = value.as_str().ok_or_else(|| {
        invalid_input_error(
            "executionMode must be one of: normal, yolo, unsafe",
            ToolGroup::ScheduledTask,
        )
    })?;
    mode.parse::<ExecutionMode>()
        .map_err(|error| invalid_input_error(&error, ToolGroup::ScheduledTask))
}

fn check_session_ownership(
    task_category: &str,
    task_session_id: Option<&str>,
    server_session_id: &str,
) -> Result<(), crate::mcp::types::MCPResult> {
    if task_category == TASK_CATEGORY_SESSION && task_session_id != Some(server_session_id) {
        Err(permission_denied_error(
            "You can only manage session callbacks for your own session. Use scheduleCallback or cancel_session_scheduled_task from the owning session.",
            ToolGroup::ScheduledTask,
        ))
    } else {
        Ok(())
    }
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

async fn resolve_assistant_id_for_session(
    session_id: &str,
) -> Result<String, crate::mcp::types::MCPResult> {
    let session = get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|error| {
            service_error_result(
                "Resolve Session",
                &format!("Failed to load session '{session_id}': {error}"),
            )
        })?
        .ok_or_else(|| {
            invalid_input_error(
                &format!("Session '{session_id}' not found"),
                ToolGroup::ScheduledTask,
            )
        })?;

    let assistant_id =
        crate::agent::extract_assistant_id_from_session(&session).ok_or_else(|| {
            invalid_input_error(
                &format!("Session '{session_id}' has no assistant configuration"),
                ToolGroup::ScheduledTask,
            )
        })?;

    validate_assistant_id(&assistant_id).await?;

    Ok(assistant_id)
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

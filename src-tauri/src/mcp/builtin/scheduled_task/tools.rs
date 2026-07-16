use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

const EXECUTION_MODE_PARAM_DESC: &str = "Tool approval mode when the task fires. \
normal: require user approval for sensitive tools. \
yolo: auto-approve standard tools (still subject to workspace/shell policy). \
unsafe: auto-approve hard-approval tools too — use only for trusted automation. \
Defaults to normal.";

fn execution_mode_workflow_steps() -> [&'static str; 2] {
    [
        "Choose executionMode: normal (default) for routine schedules; yolo only when unattended standard-tool runs are acceptable; unsafe only when hard-approval tools must run without a human present.",
        "Unsafe does not bypass workspace shell policy blocks — it only skips approval prompts.",
    ]
}

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        schedule_callback_tool(),
        create_scheduled_task_tool(),
        list_scheduled_tasks_tool(),
        get_scheduled_task_tool(),
        update_scheduled_task_tool(),
        toggle_scheduled_task_tool(),
        delete_scheduled_task_tool(),
    ]
}

fn schedule_callback_tool() -> MCPTool {
    MCPTool {
        name: "scheduleCallback".to_string(),
        title: Some("Schedule Session Callback".to_string()),
        description: tool_description(
            "Schedule a one-shot delay or recurring callback for the current session.",
            &[],
            &[
                "Provide message text to inject when the callback fires.",
                "Use delaySeconds for one-shot OR cronExpression for recurring — not both.",
                execution_mode_workflow_steps()[0],
                execution_mode_workflow_steps()[1],
            ],
            &[
                "List session callbacks via scheduled_task__listScheduledTasks if applicable.",
            ],
        )
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "message".to_string(),
                    string_prop(
                        Some(1),
                        Some(8000),
                        Some("Message to inject when the callback fires."),
                    ),
                ),
                (
                    "name".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Optional label shown in schedule lists."),
                    ),
                ),
                (
                    "delaySeconds".to_string(),
                    integer_prop(
                        Some(1),
                        Some(86400),
                        Some("One-shot delay in seconds. Mutually exclusive with cronExpression."),
                    ),
                ),
                (
                    "cronExpression".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Cron expression for recurring session callbacks. Mutually exclusive with delaySeconds."),
                    ),
                ),
                (
                    "executionMode".to_string(),
                    enum_prop_optional(
                        vec!["normal", "yolo", "unsafe"],
                        Some(EXECUTION_MODE_PARAM_DESC),
                    ),
                ),
            ],
            vec!["message".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn create_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "createScheduledTask".to_string(),
        title: Some("Create Scheduled Task".to_string()),
        description: tool_description(
            "Create a recurring scheduled task that can wake an assistant later.",
            &["Assistant configuration ID and valid cron expression."],
            &[
                "Set name, cronExpression, and assistantId.",
                execution_mode_workflow_steps()[0],
                execution_mode_workflow_steps()[1],
                "The system returns a task ID for follow-up management.",
            ],
            &[
                "Inspect with scheduled_task__getScheduledTask.",
                "Pause with scheduled_task__toggleScheduledTask.",
            ],
        )
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop(Some(1), Some(120), Some("Human-readable task name.")),
                ),
                (
                    "cronExpression".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Cron expression that defines when the task should run."),
                    ),
                ),
                (
                    "scheduleTimezone".to_string(),
                    enum_prop_optional(
                        vec!["local", "utc"],
                        Some("Schedule timezone. Defaults to local."),
                    ),
                ),
                (
                    "assistantId".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Assistant configuration ID to run."),
                    ),
                ),
                (
                    "message".to_string(),
                    string_prop(
                        Some(1),
                        Some(8000),
                        Some("Instruction message sent when the scheduled task fires."),
                    ),
                ),
                (
                    "executionMode".to_string(),
                    enum_prop_optional(
                        vec!["normal", "yolo", "unsafe"],
                        Some(EXECUTION_MODE_PARAM_DESC),
                    ),
                ),
                (
                    "workspaceOverride".to_string(),
                    string_prop(
                        None,
                        Some(4096),
                        Some("Optional absolute workspace directory to use when the task runs."),
                    ),
                ),
                (
                    "resetPlanningState".to_string(),
                    boolean_prop(Some(
                        "When true, clear goal/todo/scratchpad before injecting the task message. Does not wipe chat history. Defaults to false.",
                    )),
                ),
            ],
            vec![
                "name".to_string(),
                "cronExpression".to_string(),
                "assistantId".to_string(),
                "message".to_string(),
            ],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn list_scheduled_tasks_tool() -> MCPTool {
    MCPTool {
        name: "listScheduledTasks".to_string(),
        title: Some("List Scheduled Tasks".to_string()),
        description: tool_description(
            "List scheduled tasks, optionally filtered by assistant or enabled state.",
            &[],
            &[
                "Apply assistant or enabled filters when needed.",
                "Paginate if many tasks exist.",
            ],
            &[
                "Read details with scheduled_task__getScheduledTask.",
                "Update with scheduled_task__updateScheduledTask or toggle with scheduled_task__toggleScheduledTask.",
            ],
        )
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "assistantId".to_string(),
                    string_prop(
                        None,
                        Some(120),
                        Some("Optional assistant configuration ID filter."),
                    ),
                ),
                (
                    "enabled".to_string(),
                    boolean_prop(Some("Optional enabled-state filter.")),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn get_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "getScheduledTask".to_string(),
        title: Some("Get Scheduled Task".to_string()),
        description: tool_description(
            "Read one scheduled task in detail including message, schedule, and pinned session state.",
            &["Task ID from scheduled_task__createScheduledTask or scheduled_task__listScheduledTasks."],
            &["Pass the exact task ID."],
            &[
                "Update fields with scheduled_task__updateScheduledTask.",
                "Delete with scheduled_task__deleteScheduledTask when no longer needed.",
            ],
        )
            .to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required("Exact scheduled task ID."),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn update_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "updateScheduledTask".to_string(),
        title: Some("Update Scheduled Task".to_string()),
        description: tool_description(
            "Update mutable fields on an existing scheduled task.",
            &["Task ID from scheduled_task__listScheduledTasks or scheduled_task__getScheduledTask."],
            &[
                "Pass the task ID and only fields to change.",
                "Confirm schedule impact before saving cron changes.",
                execution_mode_workflow_steps()[0],
            ],
            &[
                "Verify with scheduled_task__getScheduledTask.",
                "Pause safely with scheduled_task__toggleScheduledTask.",
            ],
        )
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    string_prop_required("Exact scheduled task ID."),
                ),
                (
                    "name".to_string(),
                    string_prop(Some(1), Some(120), Some("New task name.")),
                ),
                (
                    "cronExpression".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("New cron expression. Recomputes the next run."),
                    ),
                ),
                (
                    "scheduleTimezone".to_string(),
                    enum_prop_optional(
                        vec!["local", "utc"],
                        Some("New schedule timezone."),
                    ),
                ),
                (
                    "assistantId".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("New assistant configuration ID."),
                    ),
                ),
                (
                    "message".to_string(),
                    string_prop(
                        Some(1),
                        Some(8000),
                        Some("Replacement message sent when the task fires."),
                    ),
                ),
                (
                    "executionMode".to_string(),
                    enum_prop_optional(
                        vec!["normal", "yolo", "unsafe"],
                        Some(EXECUTION_MODE_PARAM_DESC),
                    ),
                ),
                (
                    "workspaceOverride".to_string(),
                    string_prop(
                        None,
                        Some(4096),
                        Some("New absolute workspace override directory."),
                    ),
                ),
                (
                    "clearWorkspaceOverride".to_string(),
                    boolean_prop(Some("Set true to remove the workspace override.")),
                ),
                (
                    "resetPlanningState".to_string(),
                    boolean_prop(Some(
                        "When true, clear goal/todo/scratchpad before each run. Does not wipe chat history.",
                    )),
                ),
                (
                    "enabled".to_string(),
                    boolean_prop(Some("Updated enabled-state flag.")),
                ),
            ],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn toggle_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "toggleScheduledTask".to_string(),
        title: Some("Toggle Scheduled Task".to_string()),
        description: tool_description(
            "Enable or disable a scheduled task without changing other fields.",
            &["Task ID from scheduled_task__getScheduledTask."],
            &[
                "Pass the task ID and enabled flag.",
                "Use for safe pause/resume without editing the schedule.",
            ],
            &[
                "Confirm state with scheduled_task__getScheduledTask.",
                "Permanently remove with scheduled_task__deleteScheduledTask if obsolete.",
            ],
        )
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    string_prop_required("Exact scheduled task ID."),
                ),
                (
                    "enabled".to_string(),
                    boolean_prop(Some("Target enabled state.")),
                ),
            ],
            vec!["id".to_string(), "enabled".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn delete_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "deleteScheduledTask".to_string(),
        title: Some("Delete Scheduled Task".to_string()),
        description: tool_description(
            "Permanently delete a scheduled task.",
            &["Task ID from scheduled_task__getScheduledTask."],
            &[
                "Confirm the schedule with scheduled_task__getScheduledTask if unsure.",
                "Deletion cannot be undone.",
            ],
            &["Verify removal with scheduled_task__listScheduledTasks."],
        )
        .to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required("Exact scheduled task ID."),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

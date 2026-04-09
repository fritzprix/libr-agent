use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_scheduled_task_tool(),
        list_scheduled_tasks_tool(),
        get_scheduled_task_tool(),
        update_scheduled_task_tool(),
        toggle_scheduled_task_tool(),
        delete_scheduled_task_tool(),
    ]
}

fn create_scheduled_task_tool() -> MCPTool {
    MCPTool {
        name: "createScheduledTask".to_string(),
        title: Some("Create Scheduled Task".to_string()),
        description: "Create a recurring scheduled task that can wake an assistant later. The system generates the task ID automatically and returns it for follow-up management calls."
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
                    string_prop(Some(1), Some(120), Some("Assistant configuration ID to run.")),
                ),
                (
                    "groupId".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Optional scheduled task group ID. Provide this to join an existing group."),
                    ),
                ),
                (
                    "groupName".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Optional human-readable scheduled task group name."),
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
                    "yoloMode".to_string(),
                    boolean_prop(Some("Whether the run should execute in YOLO mode.")),
                ),
                (
                    "workspaceOverride".to_string(),
                    string_prop(
                        None,
                        Some(4096),
                        Some("Optional absolute workspace directory to use when the task runs."),
                    ),
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
        description: "List scheduled tasks, optionally filtered by assistant or enabled state. Use this to discover task IDs before reading, updating, toggling, or deleting."
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
        description: "Read one scheduled task in detail. Use this after listScheduledTasks() when you need the exact message, schedule, or pinned session state."
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
        description: "Update mutable fields on an existing scheduled task. Obtain the exact ID from createScheduledTask() or listScheduledTasks() first."
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
                    "groupId".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Optional scheduled task group ID. Provide together with groupName to move to a different group."),
                    ),
                ),
                (
                    "groupName".to_string(),
                    string_prop(
                        Some(1),
                        Some(120),
                        Some("Optional scheduled task group name."),
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
                    "yoloMode".to_string(),
                    boolean_prop(Some("Updated YOLO mode flag.")),
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
                    "clearGroup".to_string(),
                    boolean_prop(Some("Set true to remove scheduled task group metadata.")),
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
        description: "Enable or disable a scheduled task without changing its other fields. Use this for safe pause/resume control."
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
        description: "Delete a scheduled task permanently. Use getScheduledTask() first if you need to confirm the schedule before removal."
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

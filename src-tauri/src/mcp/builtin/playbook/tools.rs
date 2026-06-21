use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::schema::JSONSchema;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Helper to create a tool definition
fn create_tool_def(
    name: &str,
    title: &str,
    description: &str,
    input_schema: JSONSchema,
) -> MCPTool {
    MCPTool {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        title: Some(title.to_string()),
        output_schema: None,
        annotations: None,
    }
}

/// Schema for a playbook step
fn playbook_step_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "stepId".to_string(),
                string_prop(None, None, Some("Step ID")),
            ),
            (
                "description".to_string(),
                string_prop_required("Step description"),
            ),
            (
                "action".to_string(),
                object_prop(
                    vec![
                        ("toolName".to_string(), string_prop_required("Tool name")),
                        (
                            "purpose".to_string(),
                            string_prop_required("Purpose of using this tool"),
                        ),
                    ],
                    vec!["toolName".to_string(), "purpose".to_string()],
                    Some("Action to perform"),
                ),
            ),
            (
                "requiredData".to_string(),
                array_schema(string_prop(None, None, None), Some("Required input data")),
            ),
            (
                "outputVariable".to_string(),
                string_prop_required("Output variable name"),
            ),
        ],
        vec![
            "description".to_string(),
            "action".to_string(),
            "outputVariable".to_string(),
        ],
        Some("Playbook step definition"),
    )
}

/// Schema for success criteria
fn success_criteria_schema() -> JSONSchema {
    object_prop(
        vec![
            (
                "description".to_string(),
                string_prop_required("Success criteria description"),
            ),
            (
                "requiredArtifacts".to_string(),
                array_schema(string_prop(None, None, None), Some("Required artifacts")),
            ),
        ],
        vec!["description".to_string()],
        Some("Success criteria definition"),
    )
}

/// Create a new playbook
pub fn create_playbook_tool() -> MCPTool {
    create_tool_def(
        "createPlaybook",
        "Create Playbook",
        &tool_description(
            "Create a new reusable playbook with goal, workflow steps, and success criteria.",
            &[],
            &[
                "Define a clear goal and optional initialCommand.",
                "List workflow steps with toolName and purpose for each action.",
            ],
            &[
                "Select the playbook with playbook__selectPlaybook.",
                "List playbooks with playbook__listPlaybooks.",
            ],
        ),
        object_prop(
            vec![
                ("goal".to_string(), string_prop_required("Goal description")),
                (
                    "initialCommand".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Original command. If omitted, the playbook is stored without an initial command."),
                    ),
                ),
                (
                    "workflow".to_string(),
                    array_schema(playbook_step_schema(), Some("List of steps")),
                ),
                (
                    "successCriteria".to_string(),
                    {
                        let mut schema = success_criteria_schema();
                        schema.description = Some(
                            "Success criteria definition. If omitted, no explicit success criteria are stored."
                                .to_string(),
                        );
                        schema
                    },
                ),
            ],
            vec!["goal".to_string(), "workflow".to_string()],
            None,
        ),
    )
}

/// Select and prepare a playbook
pub fn select_playbook_tool() -> MCPTool {
    create_tool_def(
        "selectPlaybook",
        "Select Playbook",
        &tool_description(
            "Select and prepare a playbook for execution.",
            &["Playbook ID from playbook__listPlaybooks or playbook__getPlaybook."],
            &["Pass the playbook id to load it into the active workflow context."],
            &[
                "Review steps with playbook__getPlaybook.",
                "Update the playbook with playbook__updatePlaybook if steps need changes.",
            ],
        ),
        object_prop(
            vec![("id".to_string(), string_prop_required("Playbook ID"))],
            vec!["id".to_string()],
            None,
        ),
    )
}

/// List playbooks (text only)
pub fn list_playbooks_tool() -> MCPTool {
    create_tool_def(
        "listPlaybooks",
        "List Playbooks",
        &tool_description(
            "List saved playbooks with pagination and sorting.",
            &[],
            &[
                "Use page and pageSize for pagination.",
                "Sort by created_at or assistant; bookmarkFirst prioritizes bookmarked items.",
            ],
            &[
                "Open details with playbook__getPlaybook.",
                "Select for use with playbook__selectPlaybook.",
            ],
        ),
        object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Page number (1-based). If omitted, default: 1."),
                    ),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Items per page. If omitted, default: 10."),
                    ),
                ),
                (
                    "sortBy".to_string(),
                    enum_prop(
                        vec!["created_at", "assistant"],
                        "created_at",
                        Some("Sort field. Allowed values: 'created_at' or 'assistant'. If omitted, default: 'created_at'."),
                    ),
                ),
                (
                    "sortOrder".to_string(),
                    enum_prop(
                        vec!["asc", "desc"],
                        "desc",
                        Some("Sort order. Allowed values: 'asc' or 'desc'. If omitted, default: 'desc'."),
                    ),
                ),
                (
                    "bookmarkFirst".to_string(),
                    boolean_prop(Some(
                        "If true, list bookmarked playbooks before non-bookmarked ones. If omitted/false (default), use only the requested sort order.",
                    )),
                ),
            ],
            vec![],
            None,
        ),
    )
}

/// Navigate playbook UI
pub fn get_playbook_page_tool() -> MCPTool {
    create_tool_def(
        "getPlaybookPage",
        "Get Playbook Page",
        &tool_description(
            "Navigate the playbook UI listing with pagination.",
            &[],
            &["Set page and pageSize to browse the playbook catalog."],
            &[
                "Load a playbook with playbook__getPlaybook.",
                "Select a playbook with playbook__selectPlaybook.",
            ],
        ),
        object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Page number (1-based). If omitted, default: 1."),
                    ),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Items per page. If omitted, default: 10."),
                    ),
                ),
            ],
            vec![],
            None,
        ),
    )
}

/// Delete a playbook
pub fn delete_playbook_tool() -> MCPTool {
    create_tool_def(
        "deletePlaybook",
        "Delete Playbook",
        &tool_description(
            "Permanently delete a playbook by ID.",
            &["Playbook ID from playbook__listPlaybooks or playbook__getPlaybook."],
            &["Confirm the playbook is no longer needed before deleting."],
            &["List remaining playbooks with playbook__listPlaybooks."],
        ),
        object_prop(
            vec![("id".to_string(), string_prop_required("Playbook ID"))],
            vec!["id".to_string()],
            None,
        ),
    )
}

/// Get playbook details
pub fn get_playbook_tool() -> MCPTool {
    create_tool_def(
        "getPlaybook",
        "Get Playbook",
        &tool_description(
            "Get full playbook details including workflow steps and success criteria.",
            &["Playbook ID from playbook__listPlaybooks."],
            &["Pass the playbook id."],
            &[
                "Select for execution with playbook__selectPlaybook.",
                "Edit with playbook__updatePlaybook.",
            ],
        ),
        object_prop(
            vec![("id".to_string(), string_prop_required("Playbook ID"))],
            vec!["id".to_string()],
            None,
        ),
    )
}

/// Update a playbook
pub fn update_playbook_tool() -> MCPTool {
    create_tool_def(
        "updatePlaybook",
        "Update Playbook",
        &tool_description(
            "Update an existing playbook's goal, workflow, or success criteria.",
            &["Playbook ID from playbook__getPlaybook or playbook__listPlaybooks."],
            &[
                "Pass id and a playbook object with only fields to change.",
                "Omit fields to leave them unchanged.",
            ],
            &[
                "Verify changes with playbook__getPlaybook.",
                "Re-select with playbook__selectPlaybook if actively running.",
            ],
        ),
        object_prop(
            vec![
                ("id".to_string(), string_prop_required("Playbook ID")),
                (
                    "playbook".to_string(),
                    object_prop(
                        vec![
                            (
                                "goal".to_string(),
                                string_prop(
                                    None,
                                    None,
                                    Some("Goal description. If omitted, keep the current goal unchanged."),
                                ),
                            ),
                            (
                                "initialCommand".to_string(),
                                string_prop(
                                    None,
                                    None,
                                    Some("Original command. If omitted, keep the current initial command unchanged."),
                                ),
                            ),
                            (
                                "workflow".to_string(),
                                array_schema(
                                    playbook_step_schema(),
                                    Some("List of steps. If omitted, keep the current workflow unchanged."),
                                ),
                            ),
                            (
                                "successCriteria".to_string(),
                                {
                                    let mut schema = success_criteria_schema();
                                    schema.description = Some(
                                        "Success criteria definition. If omitted, keep the current success criteria unchanged."
                                            .to_string(),
                                    );
                                    schema
                                },
                            ),
                        ],
                        vec![],
                        Some("Fields to update. Omit any field you want to leave unchanged."),
                    ),
                ),
            ],
            vec!["id".to_string(), "playbook".to_string()],
            None,
        ),
    )
}

/// Returns all playbook tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_playbook_tool(),
        select_playbook_tool(),
        list_playbooks_tool(),
        get_playbook_page_tool(),
        delete_playbook_tool(),
        get_playbook_tool(),
        update_playbook_tool(),
    ]
}

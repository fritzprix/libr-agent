use crate::mcp::schema::JSONSchema;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Helper to create a tool definition
fn create_tool_def(name: &str, description: &str, input_schema: JSONSchema) -> MCPTool {
    MCPTool {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        title: None,
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
        "Create a new playbook",
        object_prop(
            vec![
                ("goal".to_string(), string_prop_required("Goal description")),
                (
                    "initialCommand".to_string(),
                    string_prop(None, None, Some("Original command")),
                ),
                (
                    "workflow".to_string(),
                    array_schema(playbook_step_schema(), Some("List of steps")),
                ),
                ("successCriteria".to_string(), success_criteria_schema()),
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
        "Select and prepare a playbook",
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
        "List playbooks (text only)",
        object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(10), None, Some("Items per page")),
                ),
                (
                    "sortBy".to_string(),
                    enum_prop(
                        vec!["created_at", "assistant"],
                        "created_at",
                        Some("Sort field"),
                    ),
                ),
                (
                    "sortOrder".to_string(),
                    enum_prop(vec!["asc", "desc"], "desc", Some("Sort order")),
                ),
                (
                    "bookmarkFirst".to_string(),
                    boolean_prop(Some("Prioritize bookmarked items")),
                ),
            ],
            vec![],
            None,
        ),
    )
}

/// Show playbooks (interactive UI)
pub fn show_playbooks_tool() -> MCPTool {
    create_tool_def(
        "showPlaybooks",
        "Show playbooks (interactive UI)",
        object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(10), None, Some("Items per page")),
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
        "Navigate playbook UI",
        object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(10), None, Some("Items per page")),
                ),
            ],
            vec!["page".to_string()],
            None,
        ),
    )
}

/// Delete a playbook
pub fn delete_playbook_tool() -> MCPTool {
    create_tool_def(
        "deletePlaybook",
        "Delete a playbook",
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
        "Get playbook details",
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
        "Update a playbook",
        object_prop(
            vec![
                ("id".to_string(), string_prop_required("Playbook ID")),
                (
                    "playbook".to_string(),
                    object_prop(
                        vec![
                            (
                                "goal".to_string(),
                                string_prop(None, None, Some("Goal description")),
                            ),
                            (
                                "initialCommand".to_string(),
                                string_prop(None, None, Some("Original command")),
                            ),
                            (
                                "workflow".to_string(),
                                array_schema(playbook_step_schema(), Some("List of steps")),
                            ),
                            ("successCriteria".to_string(), success_criteria_schema()),
                        ],
                        vec![],
                        Some("Fields to update"),
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
        show_playbooks_tool(),
        get_playbook_page_tool(),
        delete_playbook_tool(),
        get_playbook_tool(),
        update_playbook_tool(),
    ]
}

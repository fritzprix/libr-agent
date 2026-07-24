use crate::mcp::{schema::SchemaProperties, utils::schema_builder::*, MCPTool};

use crate::mcp::builtin::workspace::edit_mode::{read_file_tool_hint, PRIMARY_EDIT_TOOL};

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
use crate::mcp::builtin::workspace::edit_mode::{
    read_file_show_line_anchors_schema_hint, search_show_line_anchors_schema_hint,
};

pub fn create_read_file_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to read. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... for the canonical teamwork scaffold root. Read-only skill aliases are also available: @system-skills/..., @user-skills/..., @assistant-skills/..., and @workspace-skills/... when those roots exist for the session."),
        ),
    );

    props.insert(
        "offset".to_string(),
        integer_prop(
            None,
            None,
            Some("Starting line index (1-based or 0-based; both 0 and 1 start at the first line). Can be negative in tail mode to skip from the end (e.g. -100)."),
        ),
    );
    props.insert(
        "size".to_string(),
        integer_prop(
            None,
            None,
            Some("Number of lines to read. If negative, reads that many lines from the end of the file (tail mode)."),
        ),
    );
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(read_file_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: format!(
            "Read the contents of a file. Supports UTF-8 (with BOM), UTF-16, and Windows ANSI code pages (e.g. CP949); non-UTF-8 text is decoded instead of failing. Binary files with embedded nulls are rejected. Supports reading from a specific offset and line count (size), including negative size for tailing the end of the file. Large responses are chunked automatically to stay inline. {}",
            read_file_tool_hint()
        ),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_directory_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the directory to list. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills or @user-skills may also be listed when available."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(500),
            Some("Maximum number of items to return (default: 100, max: 500)"),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of items to skip for pagination (default: 0)"),
        ),
    );

    MCPTool {
        name: "listDirectory".to_string(),
        title: Some("List Directory".to_string()),
        description: "List files and subdirectories in a workspace directory.

- listDirectory('.') — workspace directory
- listDirectory('src/components') — subdirectory
- listDirectory('/tmp') — absolute directory

Returns names and types (file/directory). Use globFiles when you need glob-style filtering."
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

fn search_path_prop() -> crate::mcp::schema::JSONSchema {
    string_prop(
        Some(1),
        Some(1000),
        Some("Path to the file or directory to search. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills/... and @user-skills/... may also be searched when available."),
    )
}

pub fn create_glob_files_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), search_path_prop());
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Glob pattern to match file or directory names (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of matched files/directories to return (default: 50)."),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of results to skip for pagination (default: 0)."),
        ),
    );

    MCPTool {
        name: "globFiles".to_string(),
        title: Some("Glob Workspace Files".to_string()),
        description: "Find files and directories by glob pattern. Use grepFiles to search inside matches, or readFile to inspect a specific path.".to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "filePattern".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_grep_files_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), search_path_prop());
    props.insert(
        "query".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Regular expression pattern to search for text inside files. Matched against full file content with multiline mode enabled, so ^ and $ match line boundaries. '.' does not match newlines unless you opt into that in the regex itself (for example with (?s))."),
        ),
    );
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Optional glob pattern to limit which files are searched (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of matching lines to return (default: 50)."),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of matching lines to skip for pagination (default: 0)."),
        ),
    );
    props.insert(
        "ignoreCase".to_string(),
        boolean_prop(Some("Case-insensitive search (default: false).")),
    );
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(search_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "grepFiles".to_string(),
        title: Some("Grep Workspace Files".to_string()),
        description: format!(
            "Search file contents with a regex pattern. Results are line-based and paginated by matching lines. Use readFile on a hit, then {PRIMARY_EDIT_TOOL} to apply targeted edits."
        ),
        input_schema: object_schema(props, vec!["path".to_string(), "query".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Legacy combined search tool kept for backward-compatible dispatch only.
pub fn create_search_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file or directory to search. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills/... and @user-skills/... may also be searched when available."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of results to return (default: 50). For file-name search this limits matched files/directories; for content search this limits matching lines after regex expansion."),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of results to skip for pagination (default: 0)"),
        ),
    );
    props.insert(
        "query".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Regular expression pattern to search for text inside files. Matched against full file content with multiline mode enabled, so ^ and $ match line boundaries. '.' does not match newlines unless you opt into that in the regex itself (for example with (?s)). If omitted, only searches for file names."),
        ),
    );
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Glob pattern to filter files by name (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "ignoreCase".to_string(),
        boolean_prop(Some("Case-insensitive search (default: false)")),
    );
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(search_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "searchFiles".to_string(),
        title: Some("Search Workspace (Deprecated)".to_string()),
        description: "DEPRECATED: Use globFiles for filename search or grepFiles for content search. \
                     This tool is kept for backward compatibility and will be removed in a future version. \
                     Note: Requires either 'query' (content search) or 'filePattern' (filename search).".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

use crate::mcp::schema::SchemaProperties;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        navigate_tool(),
        highlight_tool(),
        install_preset_tool(),
        focus_session_tool(),
        wait_ui_tool(),
    ]
}

fn navigate_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(512),
            Some("Absolute app path starting with /"),
        ),
    );
    MCPTool {
        name: "app__navigate".to_string(),
        title: Some("Navigate App".to_string()),
        description: "Navigate the LibrAgent desktop UI to a react-router path (e.g. /mcp-servers, /agent, /settings)."
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn highlight_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "target".to_string(),
        string_prop(
            Some(1),
            Some(64),
            Some("Highlight target type; currently only \"preset\""),
        ),
    );
    props.insert(
        "name".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("Preset display name (case-insensitive)"),
        ),
    );
    props.insert(
        "ms".to_string(),
        number_prop(
            Some(0.0),
            Some(30_000.0),
            Some("Optional highlight duration in milliseconds (default 2500)"),
        ),
    );
    MCPTool {
        name: "app__highlight".to_string(),
        title: Some("Highlight UI Target".to_string()),
        description:
            "Highlight a recommended Extensions preset card (opens /mcp-servers). General UI focus helper for remote control."
                .to_string(),
        input_schema: object_schema(props, vec!["target".to_string(), "name".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn install_preset_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "name".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("Preset name (e.g. GitHub, Filesystem)"),
        ),
    );
    MCPTool {
        name: "app__install_preset".to_string(),
        title: Some("Install Extension Preset".to_string()),
        description: "One-click install a zero-config MCP Extension preset by name. Fails if the preset requires API keys or OAuth. Navigates to /mcp-servers so the install is visible."
            .to_string(),
        input_schema: object_schema(props, vec!["name".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn focus_session_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "sessionId".to_string(),
        string_prop(Some(1), Some(128), Some("Exact session storage id")),
    );
    MCPTool {
        name: "app__focus_session".to_string(),
        title: Some("Focus Agent Session".to_string()),
        description:
            "Navigate the UI to /agent/{sessionId} (e.g. after creating a session via POST /api/sessions)."
                .to_string(),
        input_schema: object_schema(props, vec!["sessionId".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn wait_ui_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "ms".to_string(),
        number_prop(
            Some(0.0),
            Some(30_000.0),
            Some("Milliseconds to sleep (default 500, max 30000)"),
        ),
    );
    props.insert(
        "ready".to_string(),
        object_prop(
            vec![
                (
                    "route".to_string(),
                    string_prop(None, None, Some("Reserved hint; use \"route\"")),
                ),
                (
                    "path".to_string(),
                    string_prop(None, None, Some("Expected path hint for operators")),
                ),
            ],
            vec![],
            Some("Optional readiness hint for operators (server still sleeps)"),
        ),
    );
    MCPTool {
        name: "app__wait_ui".to_string(),
        title: Some("Wait for UI".to_string()),
        description: "Sleep between remote UI steps for pacing. Prefer {\"ms\": 500}. Optional ready.path is a best-effort operator hint (still sleeps)."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

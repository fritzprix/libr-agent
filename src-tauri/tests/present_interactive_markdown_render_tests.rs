//! Windows-safe smoke tests for presentInteractive markdown Mermaid/LaTeX wiring.
//! Full template suite lives in `tests/integration/ui_interaction_template_tests.rs`
//! (Linux/macOS only via `integration_tests`).

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::ui::UiServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;

fn extract_html_resource(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .and_then(|content| {
            content.iter().find_map(|item| match item {
                MCPContent::Resource { resource, .. } => resource
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            })
        })
        .expect("presentInteractive should return HTML resource text")
}

#[tokio::test]
async fn present_interactive_markdown_loads_katex_and_mermaid() {
    let server = UiServer::new();
    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "title": "Diagram + Math",
                "format": "markdown",
                "content": "Inline $E=mc^2$\n\n```mermaid\nflowchart LR\n  A-->B\n```\n"
            }),
            None,
        )
        .await
        .expect("presentInteractive should render");

    assert_eq!(result.is_error, Some(false));
    let html = extract_html_resource(&result);

    assert!(
        html.contains("cdn.jsdelivr.net/npm/katex@0.16.27/dist/katex.min.js"),
        "must load KaTeX"
    );
    assert!(
        html.contains("cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js"),
        "must load Mermaid"
    );
    assert!(
        html.contains("class=\"mermaid-diagram\"") && html.contains("data-mermaid=\"true\""),
        "```mermaid fences must become diagram containers"
    );
    assert!(
        html.contains("katex.render") && html.contains("mermaid.render"),
        "client must invoke katex.render and mermaid.render"
    );
    assert!(
        html.contains("id='raw-data'") || html.contains("id=\"raw-data\""),
        "raw markdown must remain embeddable for MessageActionBar copy/export"
    );
    assert!(
        !html.contains("id=\"copy-btn\"") && !html.contains("tauri:exportMarkdownFile"),
        "copy/export chrome moved to MessageActionBar (not iframe)"
    );
}

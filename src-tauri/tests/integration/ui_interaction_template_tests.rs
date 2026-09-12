use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::ui::UiServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};

fn extract_text(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .and_then(|content| {
            content.iter().find_map(|item| match item {
                MCPContent::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn present_interactive_text_mode_includes_safe_default_options_array() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "# Report\n\nPick a direction.",
                "format": "markdown",
                "interaction": {
                    "type": "text",
                    "prompt": "What should I do next?"
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive should render");

    assert_eq!(result.is_error, Some(false));

    let text = extract_text(&result);

    let content = result
        .content
        .expect("presentInteractive should return content");
    let resource = content
        .iter()
        .find_map(|item| match item {
            MCPContent::Resource { resource, .. } => Some(resource),
            _ => None,
        })
        .expect("presentInteractive should return HTML resource");

    let html = resource["text"]
        .as_str()
        .expect("HTML resource should include inline text");

    assert!(
        html.contains("const options = [];"),
        "text-mode template should emit a valid empty options array to avoid syntax errors"
    );
    assert!(
        html.contains(
            "document.getElementById('submit-btn').addEventListener('click', handleSubmit);"
        ),
        "submit button handler should still be present in the rendered template"
    );

    assert!(
        text.contains("Content:\n# Report\n\nPick a direction."),
        "LLM-visible text should include the full content"
    );
    assert!(
        text.contains("User response required: What should I do next?"),
        "LLM-visible text should include the interaction prompt"
    );
    assert!(
        text.contains("Workflow paused until the user responds via the rendered UI."),
        "LLM-visible text should make the waiting state explicit"
    );
}

#[tokio::test]
async fn present_interactive_display_only_reminds_task_is_not_complete() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "title": "Status",
                "content": "Remaining overfull hboxes listed here.",
                "format": "markdown"
            }),
            None,
        )
        .await
        .expect("display-only presentInteractive should render");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text(&result);
    assert!(
        text.contains("Display-only UI — this does not complete the task"),
        "display-only presentInteractive must not look like task completion: {text}"
    );
    assert!(
        text.contains("ui__reportResult"),
        "display-only presentInteractive should point at reportResult for completion: {text}"
    );
    assert!(
        !text.contains("Workflow paused until the user responds"),
        "display-only mode must not claim a user response is required: {text}"
    );
}

#[tokio::test]
async fn present_interactive_select_mode_preserves_options_array() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "Choose one",
                "interaction": {
                    "type": "select",
                    "prompt": "Pick an option",
                    "options": ["alpha", "beta"]
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive select mode should render");

    let text = extract_text(&result);

    let content = result
        .content
        .expect("presentInteractive should return content");
    let resource = content
        .iter()
        .find_map(|item| match item {
            MCPContent::Resource { resource, .. } => Some(resource),
            _ => None,
        })
        .expect("presentInteractive should return HTML resource");

    let html = resource["text"]
        .as_str()
        .expect("HTML resource should include inline text");

    assert!(
        html.contains("const options = [\"alpha\",\"beta\"];"),
        "select-mode template should keep the real options array"
    );

    assert!(
        text.contains("Interaction type: select"),
        "LLM-visible text should include the interaction type"
    );
}

#[tokio::test]
async fn present_interactive_requires_interaction_type() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "Choose one",
                "interaction": {
                    "prompt": "Pick an option"
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive should return validation error");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("interaction.type"));
}

#[tokio::test]
async fn present_interactive_requires_interaction_prompt() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "Choose one",
                "interaction": {
                    "type": "text"
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive should return validation error");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("interaction.prompt"));
}

#[tokio::test]
async fn present_interactive_rejects_invalid_interaction_type() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "Choose one",
                "interaction": {
                    "type": "slider",
                    "prompt": "Pick an option"
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive should return validation error");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("Invalid interaction type"));
}

#[tokio::test]
async fn present_interactive_rejects_non_string_options() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": "Choose one",
                "interaction": {
                    "type": "select",
                    "prompt": "Pick an option",
                    "options": ["alpha", 7]
                }
            }),
            None,
        )
        .await
        .expect("presentInteractive should return validation error");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("must contain only strings"));
}

#[tokio::test]
async fn present_interactive_html_mode_sanitizes_unsafe_markup() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "content": r##"<div onclick="evil()">safe</div><script>alert('xss')</script><iframe src="https://example.com"></iframe><a href="javascript:evil()">link</a><img src="data:image/png;base64,QUJDRA==" alt="inline" width="320" height="160"><img src="https://example.com/chart.png" alt="remote"><table border="1" cellpadding="6" cellspacing="0"><tr bgcolor="#eeeeee"><td colspan="2" align="center">metric</td></tr></table>"##,
                "format": "html"
            }),
            None,
        )
        .await
        .expect("presentInteractive html mode should render");

    let content = result
        .content
        .expect("presentInteractive should return content");
    let resource = content
        .iter()
        .find_map(|item| match item {
            MCPContent::Resource { resource, .. } => Some(resource),
            _ => None,
        })
        .expect("presentInteractive should return HTML resource");

    let html = resource["text"]
        .as_str()
        .expect("HTML resource should include inline text");

    assert!(html.contains("<div>safe</div>"));
    assert!(!html.contains("alert('xss')"));
    assert!(!html.contains("<iframe src=\"https://example.com\""));
    assert!(!html.contains("onclick="));
    assert!(!html.contains("href=\"javascript:evil()\""));
    assert!(!html.contains("data:image/png;base64,QUJDRA=="));
    assert!(!html.contains("https://example.com/chart.png"));
    assert!(!html.contains("<img"));
    assert!(html.contains("<table"));
    assert!(html.contains("border=\"1\""));
    assert!(html.contains("cellpadding=\"6\""));
    assert!(html.contains("cellspacing=\"0\""));
    assert!(html.contains("<tr bgcolor=\"#eeeeee\">"));
    assert!(html.contains("<td colspan=\"2\" align=\"center\">metric</td>"));
}

#[tokio::test]
async fn report_result_renders_and_instructs_stop() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "reportResult",
            json!({
                "title": "Done",
                "status": "success",
                "criteria": "- File /workspace/answer.txt exists\n- Contains ordinal-logit summary",
                "proof": "- workspace__readFile /workspace/answer.txt returned non-empty summary",
                "result": "Wrote /workspace/answer.txt with the ordinal-logit summary."
            }),
            None,
        )
        .await
        .expect("reportResult should render");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text(&result);
    assert!(
        text.contains("STOP: Do not call any more tools"),
        "reportResult must tell the model to stop tool use: {text}"
    );
    assert!(text.contains("status=success"));
    assert!(text.contains("/workspace/answer.txt"));
    assert!(text.contains("Acceptance criteria:"));
    assert!(text.contains("Verification proof:"));
    assert!(
        text.contains("Result:\nWrote /workspace/answer.txt"),
        "summary must keep Result body extractable for checkSession: {text}"
    );

    let content = result.content.expect("reportResult should return content");
    assert_eq!(
        content.len(),
        2,
        "reportResult should return text summary + idle-stop resource marker"
    );
    assert!(
        content
            .iter()
            .any(|item| matches!(item, MCPContent::Text { .. })),
        "reportResult must include text summary"
    );
    let resource = content.iter().find_map(|item| match item {
        MCPContent::Resource { resource, .. } => Some(resource),
        _ => None,
    });
    let resource = resource.expect("reportResult must include Resource stop marker");
    let uri = resource["uri"].as_str().unwrap_or("");
    assert!(
        uri.starts_with("ui://result/"),
        "stop-marker URI should use ui://result/: {uri}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("reportResult should return structured_content");
    assert_eq!(
        structured.get("status").and_then(|v| v.as_str()),
        Some("success")
    );
    assert_eq!(
        structured.get("title").and_then(|v| v.as_str()),
        Some("Done")
    );
    assert_eq!(
        structured.get("criteria").and_then(|v| v.as_str()),
        Some("- File /workspace/answer.txt exists\n- Contains ordinal-logit summary")
    );
    assert_eq!(
        structured.get("proof").and_then(|v| v.as_str()),
        Some("- workspace__readFile /workspace/answer.txt returned non-empty summary")
    );
    assert_eq!(
        structured.get("result").and_then(|v| v.as_str()),
        Some("Wrote /workspace/answer.txt with the ordinal-logit summary.")
    );
    assert!(
        structured
            .get("deliverables")
            .and_then(|v| v.as_array())
            .is_some(),
        "deliverables array should be present"
    );
}

#[tokio::test]
async fn report_result_with_export_paths_populates_deliverables() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "reportResult",
            json!({
                "status": "success",
                "criteria": "- Generated report",
                "proof": "- File written",
                "result": "Done",
                "export_paths": ["Cargo.toml", "non_existent_file.pdf"]
            }),
            None,
        )
        .await
        .expect("reportResult should execute");

    assert_eq!(result.is_error, Some(false));
    let structured = result
        .structured_content
        .expect("structured_content expected");
    let deliverables = structured["deliverables"]
        .as_array()
        .expect("deliverables array");
    assert_eq!(deliverables.len(), 2);

    assert_eq!(deliverables[0]["path"], "Cargo.toml");
    assert_eq!(deliverables[0]["exists"], false);
    assert_eq!(deliverables[0]["absolute_path"], serde_json::Value::Null);

    assert_eq!(deliverables[1]["path"], "non_existent_file.pdf");
    assert_eq!(deliverables[1]["exists"], false);
    assert_eq!(deliverables[1]["absolute_path"], serde_json::Value::Null);
}

#[tokio::test]
async fn report_result_allows_omitting_criteria_and_proof() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "reportResult",
            json!({
                "status": "success",
                "result": "Summarized the article for the user."
            }),
            None,
        )
        .await
        .expect("reportResult without criteria/proof should succeed");
    assert_eq!(result.is_error, Some(false));

    let text = extract_text(&result);
    assert!(
        !text.contains("Acceptance criteria:"),
        "summary should omit empty criteria section: {text}"
    );
    assert!(
        !text.contains("Verification proof:"),
        "summary should omit empty proof section: {text}"
    );
    assert!(
        text.contains("Result:\nSummarized the article for the user."),
        "summary must keep Result body extractable: {text}"
    );

    let structured = result
        .structured_content
        .expect("structured_content expected");
    assert!(
        structured.get("criteria").is_none()
            || structured.get("criteria").is_some_and(|v| v.is_null()),
        "criteria should be absent/null when omitted: {structured}"
    );
    assert!(
        structured.get("proof").is_none() || structured.get("proof").is_some_and(|v| v.is_null()),
        "proof should be absent/null when omitted: {structured}"
    );
}

#[tokio::test]
async fn present_interactive_markdown_math_loads_katex_and_preserves_latex() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "presentInteractive",
            json!({
                "title": "Maxwell",
                "format": "markdown",
                "content": "Inline $E=mc^2$ and:\n\n$$\n\\begin{aligned}\n\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\epsilon_0} \\\\\n\\nabla \\cdot \\mathbf{B} &= 0\n\\end{aligned}\n$$\n\nCompare $a < b$."
            }),
            None,
        )
        .await
        .expect("presentInteractive with math should render");

    assert_eq!(result.is_error, Some(false));

    let content = result
        .content
        .expect("presentInteractive should return content");
    let resource = content
        .iter()
        .find_map(|item| match item {
            MCPContent::Resource { resource, .. } => Some(resource),
            _ => None,
        })
        .expect("presentInteractive should return HTML resource");
    let html = resource["text"]
        .as_str()
        .expect("HTML resource should include inline text");

    assert!(
        html.contains("cdn.jsdelivr.net/npm/katex@0.16.27/dist/katex.min.js"),
        "markdown results must load the KaTeX renderer script"
    );
    assert!(
        html.contains("cdn.jsdelivr.net/npm/katex@0.16.27/dist/katex.min.css"),
        "markdown results must load KaTeX CSS"
    );
    assert!(
        html.contains("katex.render") && html.contains("displayMode: displayMode"),
        "template must call katex.render for math nodes"
    );
    assert!(
        html.contains("@@MATHBLOCK_") && html.contains("math-display"),
        "markdown parser must support multiline display-math placeholders"
    );
    assert!(
        html.contains("escapeExceptPlaceholders"),
        "math must be extracted before HTML escaping"
    );
    assert!(
        html.contains("data-tex") && html.contains("data-katex-rendered"),
        "math nodes must keep source TeX and skip duplicate katex.render"
    );
    assert!(
        html.contains("<ol start=\"") || html.contains("'<ol start=\"'"),
        "ordered lists must emit start= so split lists keep markdown numbers"
    );
    assert!(
        html.contains("Cambria Math") && html.contains("font-style: italic"),
        "pre-KaTeX math fallback fonts should remain for offline/loading"
    );
    assert!(
        html.contains("replace(/<p>\\s*<\\/p>/g, '')"),
        "empty paragraph tags from math/blank-line splits must be stripped"
    );
    // Raw markdown is embedded as JSON; &= / < must still be intact there
    // (escaping happens only when the client parser emits HTML).
    assert!(
        html.contains(r"\nabla \cdot \mathbf{E} &=")
            || html.contains(r"\\nabla \\cdot \\mathbf{E} &="),
        "source markdown JSON must preserve LaTeX alignment &= before HTML escape"
    );
    assert!(
        html.contains("$a < b$") || html.contains(r"$a < b$"),
        "source markdown JSON must preserve inequality < inside inline math"
    );
}

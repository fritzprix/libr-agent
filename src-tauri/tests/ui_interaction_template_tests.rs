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

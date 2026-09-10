use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

const EMPTY_EXPORT: &str = "> *No exportable conversation messages.*\n";

pub fn messages_to_markdown(messages: &[Message]) -> String {
    if messages.is_empty() {
        return EMPTY_EXPORT.to_string();
    }

    let mut out = String::new();
    let mut started = false;
    for message in messages {
        append_message(&mut out, message, &mut started);
    }
    out
}

pub(super) fn append_message(out: &mut String, message: &Message, started: &mut bool) {
    if *started {
        out.push_str("\n\n---\n\n");
    }
    *started = true;
    out.push_str(&format_single_message(message));
}

fn format_role_header(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "tool" => "Tool",
        "system" => "System",
        other => other,
    }
}

fn format_tool_arguments(arguments_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments_json)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| arguments_json.to_string())
}

fn has_thinking_field(message: &Message) -> bool {
    message
        .thinking
        .as_deref()
        .map(str::trim)
        .is_some_and(|text| !text.is_empty())
}

fn has_tool_calls_field(message: &Message) -> bool {
    message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
}

fn format_single_message(message: &Message) -> String {
    let mut parts = vec![format!("## {}", format_role_header(&message.role))];
    let skip_content_thinking = has_thinking_field(message);
    let skip_content_tool_calls = has_tool_calls_field(message);

    if let Some(thinking) = message
        .thinking
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        parts.push(format!(
            "<details>\n<summary>Thinking</summary>\n\n{thinking}\n</details>"
        ));
    }

    let mut body_parts: Vec<String> = Vec::new();
    for item in &message.content {
        match item {
            MCPContent::Thinking { .. } if skip_content_thinking => {}
            MCPContent::ToolCall { .. } if skip_content_tool_calls => {}
            _ => {
                if let Some(formatted) = format_content_item(item) {
                    body_parts.push(formatted);
                }
            }
        }
    }

    if let Some(tool_calls) = message.tool_calls.as_ref() {
        for tool_call in tool_calls {
            let args = format_tool_arguments(&tool_call.function.arguments);
            body_parts.push(format!(
                "**Tool:** {}\n```json\n{args}\n```",
                tool_call.function.name
            ));
        }
    }

    if let Some(tool_call_id) = message
        .tool_call_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        body_parts.push(format!("*Tool Call ID: {tool_call_id}*"));
    }

    if let Some(display_message) = error_display_message(message.error.as_ref()) {
        body_parts.push(format!("**Error:** {display_message}"));
    }

    let body = body_parts.join("\n\n").trim().to_string();
    if !body.is_empty() {
        parts.push(body);
    }

    parts.join("\n\n")
}

fn format_content_item(item: &MCPContent) -> Option<String> {
    match item {
        MCPContent::Text { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        MCPContent::Thinking { thinking, .. } => {
            let trimmed = thinking.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(format!(
                    "<details>\n<summary>Thinking</summary>\n\n{trimmed}\n</details>"
                ))
            }
        }
        MCPContent::ToolCall {
            name, arguments, ..
        } => {
            let args = format_tool_arguments(arguments);
            Some(format!("**Tool:** {name}\n```json\n{args}\n```"))
        }
        MCPContent::Image { mime_type, .. } => Some(format!("[Image: {mime_type}]")),
        MCPContent::Audio { mime_type, .. } => Some(format!("[Audio: {mime_type}]")),
        MCPContent::Resource { resource, .. } => {
            let mime_type = resource
                .get("mimeType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let uri = resource.get("uri").and_then(serde_json::Value::as_str);
            Some(match uri {
                Some(uri) => format!("[UI Resource: {mime_type} - {uri}]"),
                None => format!("[UI Resource: {mime_type}]"),
            })
        }
    }
}

fn error_display_message(error: Option<&serde_json::Value>) -> Option<String> {
    let error = error?;
    error
        .get("displayMessage")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

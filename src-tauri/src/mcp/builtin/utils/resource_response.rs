use crate::mcp::types::{MCPContent, MCPResult, ServiceInfo};
use serde_json::json;

/// Creates a standardized UI resource response with service information.
///
/// This helper ensures all UI resources include proper service metadata
/// for correct routing of user interactions on the frontend.
///
/// # Arguments
/// * `uri` - The resource URI (e.g., "ui://prompt/123")
/// * `mime_type` - The MIME type (typically "text/html")
/// * `html` - The rendered HTML content
/// * `server_name` - The name of the server (e.g., "ui", "playbook")
/// * `tool_name` - The name of the tool (e.g., "presentInteractive", "visualizeData")
/// * `message` - Optional text message to prepend before the resource
///
/// # Returns
/// An `MCPResult` containing the resource with embedded `ServiceInfo`
pub fn create_resource_response(
    uri: &str,
    mime_type: &str,
    html: &str,
    server_name: &str,
    tool_name: &str,
    message: Option<&str>,
) -> MCPResult {
    let service_info = ServiceInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        backend_type: "BuiltInRust".to_string(),
    };

    let mut content = Vec::new();

    // Add text message if provided (for workspace tools)
    if let Some(msg) = message {
        content.push(MCPContent::Text {
            text: msg.to_string(),
        });
    }

    // Add resource content
    content.push(MCPContent::Resource {
        resource: json!({
            "uri": uri,
            "mimeType": mime_type,
            "text": html,
        }),
        service_info,
    });

    MCPResult {
        content: Some(content),
        structured_content: None,
        is_error: Some(false),
    }
}

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, MCPContent, MCPResult, MCPTool, ServiceContext, ServiceContextOptions,
};
use crate::mcp::utils::schema_builder::*;

pub mod guides;
pub mod platform;

/// Bootstrap Server - Platform detection and development tool installation guides
///
/// This server is stateless and provides:
/// - Platform detection (OS, architecture, shell)
/// - Installation guides for common development tools
#[derive(Debug)]
pub struct BootstrapServer;

impl BootstrapServer {
    pub fn new() -> Self {
        Self
    }

    /// Detect the current platform
    fn detect_platform(&self) -> MCPResult {
        let platform = platform::detect_current_platform();

        MCPResult {
            content: Some(vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&platform).unwrap(),
            }]),
            structured_content: Some(json!(platform)),
            is_error: Some(false),
        }
    }

    /// Get installation guide for a development tool
    fn get_bootstrap_guide(&self, args: Value) -> MCPResult {
        let tool = args.get("tool").and_then(|v| v.as_str()).unwrap_or("");

        let platform = args.get("platform").and_then(|v| v.as_str());

        if tool.is_empty() {
            return MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: "Error: 'tool' parameter is required".to_string(),
                }]),
                structured_content: None,
                is_error: Some(true),
            };
        }

        let guide = guides::get_installation_guide(tool, platform);

        MCPResult {
            content: Some(vec![MCPContent::Text {
                text: format!(
                    "Installation guide for {} on {}:\n{}",
                    guide.tool,
                    guide.platform,
                    serde_json::to_string_pretty(&guide).unwrap()
                ),
            }]),
            structured_content: Some(json!(guide)),
            is_error: Some(false),
        }
    }
}

impl Default for BootstrapServer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuiltinMCPServer for BootstrapServer {
    fn name(&self) -> &str {
        "bootstrap"
    }

    fn description(&self) -> &str {
        "Platform detection and development tool installation guides"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn display_name(&self) -> String {
        "Bootstrap Server".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: self.display_name(),
            description: self.description().to_string(),
            icon: Some("🚀".to_string()),
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            create_detect_platform_tool(),
            create_get_bootstrap_guide_tool(),
        ]
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: String::new(),
            structured_state: None,
        }
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!("Bootstrap server tool called: {}", tool_name);

        match tool_name {
            "detectPlatform" | "builtin_bootstrap__detectPlatform" => Ok(self.detect_platform()),
            "getBootstrapGuide" | "builtin_bootstrap__getBootstrapGuide" => {
                Ok(self.get_bootstrap_guide(args))
            }
            _ => Err(format!(
                "Unknown tool: {}. Available tools: detectPlatform, getBootstrapGuide",
                tool_name
            )),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        // Bootstrap server is stateless, no context switching needed
        Ok(())
    }
}

/// Create the detectPlatform tool definition
fn create_detect_platform_tool() -> MCPTool {
    MCPTool {
        name: "detectPlatform".to_string(),
        title: Some("Detect Platform".to_string()),
        description: "Detect current operating system, architecture, and shell environment"
            .to_string(),
        input_schema: object_schema(HashMap::new(), vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create the getBootstrapGuide tool definition
fn create_get_bootstrap_guide_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "tool".to_string(),
        enum_prop_required(
            vec!["node", "python", "uv", "docker", "git"],
            "Tool to install",
        ),
    );
    props.insert(
        "platform".to_string(),
        enum_prop(
            vec!["windows", "linux", "darwin", "auto"],
            "auto",
            Some("Target platform (auto-detect if omitted)"),
        ),
    );

    MCPTool {
        name: "getBootstrapGuide".to_string(),
        title: Some("Get Bootstrap Guide".to_string()),
        description:
            "Get installation guide for common development tools (node, python, uv, docker, git)"
                .to_string(),
        input_schema: object_schema(props, vec!["tool".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_platform() {
        let server = BootstrapServer::new();
        let result = server.call_tool("detectPlatform", json!({})).await;

        assert!(result.is_ok());
        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.is_error, Some(false));
        assert!(mcp_result.structured_content.is_some());
    }

    #[tokio::test]
    async fn test_get_bootstrap_guide() {
        let server = BootstrapServer::new();
        let result = server
            .call_tool(
                "getBootstrapGuide",
                json!({
                    "tool": "node",
                    "platform": "windows"
                }),
            )
            .await;

        assert!(result.is_ok());
        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.is_error, Some(false));
        assert!(mcp_result.structured_content.is_some());

        let guide = mcp_result.structured_content.unwrap();
        assert_eq!(guide["tool"], "node");
        assert_eq!(guide["platform"], "windows");
    }

    #[tokio::test]
    async fn test_get_bootstrap_guide_missing_tool() {
        let server = BootstrapServer::new();
        let result = server.call_tool("getBootstrapGuide", json!({})).await;

        assert!(result.is_ok());
        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let server = BootstrapServer::new();
        let result = server.call_tool("unknownTool", json!({})).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_server_metadata() {
        let server = BootstrapServer::new();
        assert_eq!(server.name(), "bootstrap");
        assert_eq!(server.version(), "1.0.0");
        assert!(!server.description().is_empty());
    }

    #[test]
    fn test_tools_list() {
        let server = BootstrapServer::new();
        let tools = server.tools();
        assert_eq!(tools.len(), 2);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"detectPlatform"));
        assert!(tool_names.contains(&"getBootstrapGuide"));
    }
}

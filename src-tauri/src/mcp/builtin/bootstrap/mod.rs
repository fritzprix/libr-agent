use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext, ServiceContextOptions,
};
use crate::mcp::utils::schema_builder::*;

pub mod guides;
pub mod platform;

/// Bootstrap Server - Platform detection and development tool installation guides
///
/// This server is stateless and provides:
/// - Platform detection (OS, architecture, shell)
/// - Installation guides for common development tools (node, python, uv, docker, git)
///
/// Note: This server can be disabled through agent configuration's allowedBuiltInServiceAliases
#[derive(Debug)]
pub struct BootstrapServer {
    platform_cache: Arc<RwLock<Option<(platform::PlatformInfo, Instant)>>>,
}

impl BootstrapServer {
    pub fn new() -> Self {
        Self {
            platform_cache: Arc::new(RwLock::new(None)),
        }
    }

    #[allow(dead_code)]
    fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.platform_cache.write() {
            *cache = None;
        }
    }

    /// Detect the current platform
    fn detect_platform(&self) -> MCPResult {
        let platform = platform::detect_current_platform();

        let text = format!(
            "✓ Platform detected:\n\n\
             OS: {}\n\
             Architecture: {}\n\
             Shell: {}\n\
             Home Directory: {}\n\
             Temp Directory: {}",
            platform.os,
            platform.arch,
            platform.shell,
            platform.home_dir.as_deref().unwrap_or("N/A"),
            platform.temp_dir
        );

        let hint = SuccessHint::new(
            text,
            vec![
                "Use getBootstrapGuide(tool) to get installation instructions".to_string(),
                "Available tools: node, python, uv, docker, git".to_string(),
            ],
        );

        hint.to_mcp_result_with_data(Some(json!(platform)))
    }

    /// Get installation guide for a development tool
    fn get_bootstrap_guide(&self, args: Value) -> MCPResult {
        let tool = match args.get("tool").and_then(|v| v.as_str()) {
            Some(t) => {
                if t.trim().is_empty() {
                    return invalid_input_error("Tool name cannot be empty", ToolGroup::Bootstrap);
                }
                t
            }
            None => return missing_param_error("tool", ToolGroup::Bootstrap),
        };

        // Validate tool name
        let valid_tools = ["node", "python", "uv", "docker", "git"];
        if !valid_tools.contains(&tool) {
            return invalid_input_error(
                &format!(
                    "Invalid tool '{}'. Must be one of: {}",
                    tool,
                    valid_tools.join(", ")
                ),
                ToolGroup::Bootstrap,
            );
        }

        let platform = args.get("platform").and_then(|v| v.as_str());

        // Validate platform if provided
        if let Some(p) = platform {
            let valid_platforms = ["windows", "linux", "darwin", "auto"];
            if !valid_platforms.contains(&p) {
                return invalid_input_error(
                    &format!(
                        "Invalid platform '{}'. Must be one of: {}",
                        p,
                        valid_platforms.join(", ")
                    ),
                    ToolGroup::Bootstrap,
                );
            }
        }

        let guide = guides::get_installation_guide(tool, platform);
        let formatted_text = guide.format_as_text();

        let hint = SuccessHint::new(
            formatted_text,
            vec![
                format!("Run: {} to verify installation", guide.verification),
                "Use detectPlatform to check your current environment".to_string(),
            ],
        );

        hint.to_mcp_result_with_data(Some(json!(guide)))
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
        const CACHE_TTL_SECS: u64 = 30; // Platform rarely changes

        // Check cache first
        if let Ok(cache_guard) = self.platform_cache.read() {
            if let Some((platform, last_update)) = cache_guard.as_ref() {
                if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                    return ServiceContext {
                        context_prompt: format!(
                            "## Bootstrap\n\nCurrent platform: {} ({}) using {}",
                            platform.os, platform.arch, platform.shell
                        ),
                        structured_state: Some(json!(platform)),
                    };
                }
            }
        }

        // Cache miss - detect platform
        let platform = platform::detect_current_platform();

        // Update cache
        if let Ok(mut cache_guard) = self.platform_cache.write() {
            *cache_guard = Some((platform.clone(), Instant::now()));
        }

        ServiceContext {
            context_prompt: format!(
                "## Bootstrap\n\nCurrent platform: {} ({}) using {}",
                platform.os, platform.arch, platform.shell
            ),
            structured_state: Some(json!(platform)),
        }
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
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
        description: "Detect current operating system, architecture, and shell environment

Use this tool to:
• Identify platform-specific requirements before installation
• Verify system compatibility with development tools
• Get accurate environment information for troubleshooting

Returns: OS type (windows/darwin/linux), CPU architecture (x64/arm64), default shell, home directory path, and temp directory path

💡 Next Steps:
• Use getBootstrapGuide(tool) to get installation instructions for your detected platform
• Available tools: node, python, uv, docker, git"
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
            "Development tool to install (node, python, uv, docker, git)",
        ),
    );
    props.insert(
        "platform".to_string(),
        enum_prop(
            vec!["windows", "linux", "darwin", "auto"],
            "auto",
            Some(
                "Target platform (auto = detect automatically, windows = Windows, darwin = macOS, linux = Linux)",
            ),
        ),
    );

    MCPTool {
        name: "getBootstrapGuide".to_string(),
        title: Some("Get Bootstrap Guide".to_string()),
        description: "Get step-by-step installation guide for common development tools

Supported Tools:
• node - Node.js runtime and npm package manager
• python - Python interpreter and pip
• uv - Ultra-fast Python package installer
• docker - Docker container platform
• git - Version control system

The guide includes:
• Platform-specific installation commands
• Download URLs for installers
• Verification commands to test installation
• Post-installation notes and configuration tips

💡 Workflow:
1. (Optional) Call detectPlatform to identify your system
2. Call getBootstrapGuide(tool, platform) to get instructions
3. Follow the numbered steps in the response
4. Run verification command to confirm installation"
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
        let result = server.call_tool("detectPlatform", json!({}), None).await;

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
                None,
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
        let result = server.call_tool("getBootstrapGuide", json!({}), None).await;

        assert!(result.is_ok());
        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let server = BootstrapServer::new();
        let result = server.call_tool("unknownTool", json!({}), None).await;

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

    #[tokio::test]
    async fn test_detect_platform_formatted_output() {
        let server = BootstrapServer::new();
        let result = server
            .call_tool("detectPlatform", json!({}), None)
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(false));

        let content = result.content.unwrap();
        let text = match &content[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };

        // Verify visual markers
        assert!(text.contains("✓ Platform detected"));
        // Verify labeled fields
        assert!(text.contains("OS:"));
        assert!(text.contains("Architecture:"));
        assert!(text.contains("Shell:"));
        // Verify guidance marker
        assert!(text.contains("💡 Next"));
    }

    #[tokio::test]
    async fn test_get_bootstrap_guide_formatted_output() {
        let server = BootstrapServer::new();
        let result = server
            .call_tool(
                "getBootstrapGuide",
                json!({"tool": "node", "platform": "windows"}),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(false));

        let content = result.content.unwrap();
        let text = match &content[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };

        // Verify visual markers
        assert!(text.contains("✓ Installation guide"));
        // Verify numbered steps
        assert!(text.contains("1."));
        // Verify command prefix
        assert!(text.contains("$"));
        // Verify verification section
        assert!(text.contains("📋 Verification"));
        // Verify notes section
        assert!(text.contains("📝 Notes"));
    }

    #[tokio::test]
    async fn test_empty_tool_name_validation() {
        let server = BootstrapServer::new();
        let result = server
            .call_tool("getBootstrapGuide", json!({"tool": "   "}), None)
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));

        let content = result.content.unwrap();
        let text = match &content[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };

        assert!(text.contains("Tool name cannot be empty"));
    }

    #[tokio::test]
    async fn test_service_context_provides_platform() {
        let server = BootstrapServer::new();
        let context = server.get_service_context(None).await;

        assert!(!context.context_prompt.is_empty());
        assert!(context.context_prompt.contains("## Bootstrap"));
        assert!(context.context_prompt.contains("Current platform:"));
        assert!(context.structured_state.is_some());
    }

    #[tokio::test]
    async fn test_service_context_caching() {
        let server = BootstrapServer::new();

        // First call
        let context1 = server.get_service_context(None).await;
        let text1 = context1.context_prompt.clone();

        // Second call (should use cache)
        let context2 = server.get_service_context(None).await;
        let text2 = context2.context_prompt;

        assert_eq!(text1, text2);
    }
}

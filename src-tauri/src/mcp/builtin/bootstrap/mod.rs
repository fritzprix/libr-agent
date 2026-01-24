use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::mcp::builtin::error_guidance::{
    missing_param_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};

pub mod guides;
pub mod platform;
mod tools;

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

    /// Build service context from platform info
    fn build_service_context(platform: &platform::PlatformInfo) -> ServiceContext {
        let mut context_parts = vec![
            format!("Platform: {} ({})", platform.os, platform.arch),
            format!("Shell: {}", platform.shell),
        ];

        if let Some(distro) = &platform.distro {
            context_parts.push(format!("Distribution: {} ({})", distro.name, distro.id));
        }

        if let Some(pm) = &platform.package_manager {
            context_parts.push(format!("Package Manager: {}", pm));
        }

        let installed: Vec<String> = platform
            .installed_tools
            .iter()
            .filter(|(_, info)| info.installed)
            .map(|(name, _)| name.clone())
            .collect();

        if !installed.is_empty() {
            context_parts.push(format!("Installed Tools: {}", installed.join(", ")));
        }

        ServiceContext {
            context_prompt: format!("## Bootstrap\n\n{}", context_parts.join("\n")),
            structured_state: Some(json!(platform)),
        }
    }

    /// Detect the current platform
    fn detect_platform(&self) -> MCPResult {
        let platform = platform::detect_current_platform();

        let mut sections = vec![
            format!("OS: {}", platform.os),
            format!("Architecture: {}", platform.arch),
            format!("Shell: {}", platform.shell),
            format!(
                "Home Directory: {}",
                platform.home_dir.as_deref().unwrap_or("N/A")
            ),
            format!("Temp Directory: {}", platform.temp_dir),
        ];

        // Add Linux distribution info
        if let Some(distro) = &platform.distro {
            sections.push("\nLinux Distribution:".to_string());
            sections.push(format!("  Name: {}", distro.name));
            sections.push(format!("  ID: {}", distro.id));
            if let Some(version) = &distro.version {
                sections.push(format!("  Version: {}", version));
            }
        }

        // Add package manager info
        if let Some(pm) = &platform.package_manager {
            sections.push(format!("\nPackage Manager: {}", pm));
        }

        // Add installed tools summary
        let installed: Vec<&String> = platform
            .installed_tools
            .iter()
            .filter(|(_, info)| info.installed)
            .map(|(name, _)| name)
            .collect();

        let missing: Vec<&String> = platform
            .installed_tools
            .iter()
            .filter(|(_, info)| !info.installed)
            .map(|(name, _)| name)
            .collect();

        if !installed.is_empty() {
            sections.push(format!("\nInstalled Tools ({}):", installed.len()));
            for tool in &installed {
                if let Some(info) = platform.installed_tools.get(*tool) {
                    let version = info.version.as_deref().unwrap_or("unknown");
                    sections.push(format!("  ✓ {}: {}", tool, version));
                }
            }
        }

        if !missing.is_empty() {
            sections.push(format!("\nMissing Tools ({}):", missing.len()));
            for tool in &missing {
                sections.push(format!("  ✗ {} (Use: getBootstrapGuide('{}'))", tool, tool));
            }
        }

        let text = format!("✓ Platform detected:\n\n{}", sections.join("\n"));

        let mut next_steps = vec![];
        if !missing.is_empty() {
            next_steps.push("Use getBootstrapGuide(tool) to install missing tools".to_string());
        }
        next_steps.push("Available guides: node, python, uv, docker, git".to_string());

        let hint = SuccessHint::new(text, next_steps);

        hint.to_mcp_result_with_data(Some(json!(platform)))
    }

    /// Get installation guide for a development tool
    fn get_bootstrap_guide(&self, args: Value) -> MCPResult {
        let tool = match args.get("tool").and_then(|v| v.as_str()) {
            Some(t) => {
                if t.trim().is_empty() {
                    return ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        "Tool name cannot be empty".to_string(),
                        vec![
                            "Use detectPlatform to identify missing tools".to_string(),
                            "Valid tools: node, python, uv, docker, git".to_string(),
                        ],
                        ToolGroup::Bootstrap,
                    )
                    .to_mcp_result();
                }
                t
            }
            None => return missing_param_error("tool", ToolGroup::Bootstrap),
        };

        // Validate tool name
        let valid_tools = ["node", "python", "uv", "docker", "git"];
        if !valid_tools.contains(&tool) {
            return ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid tool '{}'. Must be one of: {}",
                    tool,
                    valid_tools.join(", ")
                ),
                vec![
                    "Use detectPlatform first to identify missing tools".to_string(),
                    "Valid tools: node, python, uv, docker, git".to_string(),
                ],
                ToolGroup::Bootstrap,
            )
            .to_mcp_result();
        }

        let platform = args.get("platform").and_then(|v| v.as_str());

        // Validate platform if provided
        if let Some(p) = platform {
            let valid_platforms = ["windows", "linux", "darwin", "auto"];
            if !valid_platforms.contains(&p) {
                return ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Invalid platform '{}'. Must be one of: {}",
                        p,
                        valid_platforms.join(", ")
                    ),
                    vec![
                        "Use 'auto' to detect platform automatically".to_string(),
                        "Use detectPlatform to see your current platform".to_string(),
                    ],
                    ToolGroup::Bootstrap,
                )
                .to_mcp_result();
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
        tools::all_tools()
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL_SECS: u64 = 30; // Platform rarely changes

        // Check cache first
        if let Ok(cache_guard) = self.platform_cache.read() {
            if let Some((platform, last_update)) = cache_guard.as_ref() {
                if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                    return Self::build_service_context(platform);
                }
            }
        }

        // Cache miss - detect platform
        let platform = platform::detect_current_platform();

        // Update cache
        if let Ok(mut cache_guard) = self.platform_cache.write() {
            *cache_guard = Some((platform.clone(), Instant::now()));
        }

        Self::build_service_context(&platform)
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::debug!("Bootstrap server tool called: {}", tool_name);

        match tool_name {
            "detectPlatform" => Ok(self.detect_platform()),
            "getBootstrapGuide" => Ok(self.get_bootstrap_guide(args)),
            _ => Err(format!(
                "Unknown tool: {}. Available tools: detectPlatform, getBootstrapGuide",
                tool_name
            )),
        }
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
        assert!(context.context_prompt.contains("Platform:"));
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

    #[tokio::test]
    async fn test_detect_platform_dual_channel_compliance() {
        // Section 4 - The Response Standard: Dual-Channel Rule
        let server = BootstrapServer::new();
        let result = server
            .call_tool("detectPlatform", json!({}), None)
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(false));

        // Validate text content is self-sufficient (Section 4.1)
        let content = result.content.unwrap();
        let text = match &content[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };

        // Section 4.2 - Narrative Requirement: Text must tell the full story
        assert!(text.contains("OS:"));
        assert!(text.contains("Architecture:"));
        assert!(text.contains("Shell:"));

        // Validate inline guidance for missing tools
        if text.contains("Missing Tools") {
            assert!(
                text.contains("Use: getBootstrapGuide"),
                "Missing tools should include inline guidance"
            );
        }

        // Validate next steps guidance exists
        assert!(
            text.contains("💡 Next") || text.contains("Available guides"),
            "Should provide next steps guidance"
        );

        // Validate structured_content exists for UI rendering
        assert!(result.structured_content.is_some());
    }

    #[tokio::test]
    async fn test_error_guidance_compliance() {
        // Section 6 - Error Handling: The "Success Hint" Pattern
        let server = BootstrapServer::new();
        let result = server
            .call_tool("getBootstrapGuide", json!({"tool": "invalid_tool"}), None)
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));

        let content = result.content.unwrap();
        let text = match &content[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };

        // Section 6.1 - The Detour Principle: Error with solution
        assert!(
            text.contains("Invalid tool"),
            "Error message should be clear"
        );
        assert!(
            text.contains("Use detectPlatform") || text.contains("Valid tools"),
            "Error should provide recovery hints"
        );

        // Section 6.2 - Tool Group Isolation: Suggestions from same domain
        assert!(
            text.contains("detectPlatform") || text.contains("getBootstrapGuide"),
            "Should only suggest Bootstrap tools"
        );
        assert!(
            !text.contains("listServers") && !text.contains("createServer"),
            "Should not suggest MCP Manager tools"
        );
    }
}

use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;
use std::collections::HashMap;

/// Detect current system platform and installed tools
pub fn detect_platform_tool() -> MCPTool {
    MCPTool {
        name: "detectPlatform".to_string(),
        title: Some("Detect Platform".to_string()),
        description:
            "Comprehensively detect current system environment and installed development tools

Use this tool to:
• Identify platform-specific requirements before installation
• Check which development tools are already installed
• Determine the appropriate package manager for your system
• Get Linux distribution details (Debian, Ubuntu, Arch, Fedora, etc.)
• Verify system compatibility with development tools
• Get accurate environment information for troubleshooting

Returns:
• OS type (windows/darwin/linux)
• CPU architecture (x64/arm64/arm)
• Default shell (bash/zsh/powershell/etc.)
• Linux distribution info (name, ID, version) if applicable
• Available package manager (apt/dnf/pacman/brew/etc.)
• Installed tools with versions (node, python, docker, git, cargo, etc.)
• Missing tools that can be installed
• Home and temp directory paths

💡 Next Steps:
• Review installed tools - no need to reinstall what you already have
• Use getBootstrapGuide(tool) only for missing tools
• Available guides: node, python, uv, docker, git"
                .to_string(),
        input_schema: object_schema(HashMap::new(), vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Get installation guide for a development tool
pub fn get_bootstrap_guide_tool() -> MCPTool {
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

/// Returns all bootstrap tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![detect_platform_tool(), get_bootstrap_guide_tool()]
}

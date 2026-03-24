# MCP Built-in Server Module Development Guide

The Rust MCP server in LibrAgent is modularized for extensibility. This guide explains how to add new MCP server modules with concrete examples.

## 📁 Current Module Structure

```text
src-tauri/src/mcp/builtin/
├── mod.rs                # Server trait definitions and registry
├── agent/                # Agent role and session management
├── attachments/          # Session-scoped file attachment and search
├── bootstrap/            # Shared initialization and bootstrap helpers
├── browser/              # Headless browser automation
├── browser_content_store.rs # Browser-aware content store bridge
├── error_guidance.rs     # Error analysis and guidance utilities
├── knowledge/            # Semantic search and memory
├── mcp_manager/          # MCP server management
├── planning/             # Task planning and tracking
├── playbook/             # Workflow automation
├── skills/               # Reusable capabilities
├── tests/                # Integration tests
├── ui/                   # UI interaction tools
├── workspace/            # Terminal, File Manager, Code Execution
├── utils.rs              # Common utilities
└── README.md             # This guide
```

## 🏗️ MCP Server Module Architecture

### Core Trait: `BuiltinMCPServer`

All MCP servers must implement the following trait:

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;                          // Server name (e.g., "workspace")
    fn description(&self) -> &str;                   // Server description
    fn version(&self) -> &str { "1.0.0" }           // Version (default provided)

    // Returns a human-friendly display name for the UI
    // Default implementation capitalizes the server name
    fn display_name(&self) -> String { ... }

    // Returns complete UI metadata for this server
    // Default implementation uses display_name and description
    fn metadata(&self) -> BuiltinServerMetadata { ... }

    // Returns a list of tools provided by this server
    fn tools(&self) -> Vec<MCPTool>;

    // Executes a tool
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>
    ) -> Result<MCPResult, String>;

    // Returns a markdown-formatted string describing the server's current status and context.
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(
                "## {}\n**Description**: {}",
                self.display_name(),
                self.description()
            ),
            structured_state: None,
        }
    }
}
```

### Tool Naming Conventions

- **Internal (Rust Module)**: Use simple names (e.g., `"echo"`, `"readFile"`). The server implementation expects these.
- **External (Agent/Frontend via Proxy)**: Tools are exposed and called with the `builtin_{server}__{tool}` prefix (e.g., `builtin_example__echo`). The `MCPServiceProxy` handles routing and strips the prefix before invoking the internal tool.

## 🚀 Step-by-Step Guide to Adding a New MCP Server Module

### Step 1: Create a New Server File

Example: Create a new file, e.g., `src-tauri/src/mcp/builtin/example.rs`.

```rust
// src-tauri/src/mcp/builtin/example.rs

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, error};

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, MCPContent};
use crate::mcp::MCPTool;
use crate::mcp::utils::schema_builder::*; // Helper functions for schema building

/// Example MCP Server providing text processing tools
#[derive(Debug)]
pub struct ExampleServer;

impl ExampleServer {
    pub fn new() -> Self {
        Self
    }

    /// Define the Echo tool using schema builder helpers
    fn create_echo_tool() -> MCPTool {
        let mut props = HashMap::new();
        props.insert(
            "text".to_string(),
            string_prop(
                Some(1),
                Some(1000),
                Some("Text to echo back to the user"),
            ),
        );

        MCPTool {
            name: "echo".to_string(),
            title: Some("Echo Text".to_string()),
            description: "Echo the input text back to the user".to_string(),
            input_schema: object_schema(props, vec!["text".to_string()]),
            output_schema: None,
            annotations: None,
        }
    }

    /// Handle Echo tool execution
    async fn handle_echo(&self, args: Value) -> Result<MCPResult, String> {
        let text = args.get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter 'text'".to_string())?;

        info!("Echo tool called with text: {}", text);

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: text.to_string(),
                is_error: None,
            }]),
            is_error: Some(false),
            structured_content: None,
        })
    }
}

#[async_trait]
impl BuiltinMCPServer for ExampleServer {
    fn name(&self) -> &str {
        "example"
    }

    fn description(&self) -> &str {
        "Example MCP server providing text processing tools"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![Self::create_echo_tool()]
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "echo" => self.handle_echo(args).await,
            _ => Err(format!("Tool '{}' not found", tool_name)),
        }
    }
}
```

### Step 2: Register in `mod.rs`

```rust
// src-tauri/src/mcp/builtin/mod.rs

pub mod example; // Add module

// In BuiltinServerRegistry::new_with_session_manager (or similar)
registry.register_server(Box::new(example::ExampleServer::new()));
```

## 🔧 Frontend Integration

### Automatic Tool Detection

The frontend automatically detects new tools exposed by the registry via `MCPServiceProxy`. The proxy exposes tools with the format `builtin_{server}__{tool}`.

### Tool Call Example

If you are calling the tool via `agentCallBuiltinTool` (which wraps the Rust backend call):

```typescript
const response = await agentCallBuiltinTool(
  sessionId,
  'builtin_example__echo', // Must use the prefixed name for routing
  { text: "Hello, LibrAgent!" }
);
```

Or if executing a tool call object (e.g. from LLM response):

```typescript
const toolCall = {
  id: "req-123",
  type: "function",
  function: {
    name: "builtin_example__echo", // Prefixed name required by proxy
    arguments: JSON.stringify({ text: "Hello, LibrAgent!" })
  }
};

const response = await executeToolCall(toolCall);
```

## 🛡️ Security Considerations

- Validate all inputs.
- Use `crate::mcp::builtin::utils::SecurityValidator` for file system operations.
- Ensure proper error handling and logging using `tracing`.

---

**Note**: This guide is based on the current architecture of LibrAgent.

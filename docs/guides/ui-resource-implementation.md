# UI Resource Implementation Guide

## Overview

This guide explains how to implement UI resources in LibrAgent. UI resources allow MCP tools (both Rust-based built-in servers and external MCP servers) to return interactive HTML interfaces rendered safely inside sandboxed iframes via `@mcp-ui/client`. User actions inside these interfaces can trigger tool calls, user messages, intents, external link opens, or system notifications.

## Architecture

### Component Flow

```text
UIResourceRenderer (@mcp-ui/client iframe)
  → onUIAction callback
    → AgentMessageRenderer / ContentItemRenderer
      → useUIActionHandler hook
        → handleUserToolCall (src/lib/backend.ts)
          → Rust Backend (Tauri invoke: execute_user_tool_call)
            → Builtin MCP Server (Rust) / External MCP Server
```

### Key Components

1. **UIResourceRenderer** (`@mcp-ui/client` in `ContentItemRenderer.tsx`)
   - Renders HTML content in a sandboxed iframe with theme styles injected (`applyThemeToUiResource`).
   - Captures user interactions (button clicks, form submits) and dispatches `onUIAction` events to the parent React context.

2. **AgentMessageRenderer & ContentItemRenderer** (`src/features/agent/components/AgentMessageRenderer/`)
   - Renders message history and content items (including `resource` items with `mimeType: "text/html"` and `uri: "ui://..."`).
   - Hooks up `onUIAction` to `useUIActionHandler`.

3. **useUIActionHandler** (`src/features/agent/components/AgentMessageRenderer/hooks/useUIActionHandler.ts`)
   - Handles `UIActionResult` events from `@mcp-ui/client`.
   - Supports multiple action types:
     - `tool`: Invokes an MCP tool (`server__tool`) via Rust backend (`handleUserToolCall`) or internal Tauri action (`tauri:*` via `executeUiTauriAction`).
     - `intent` / `prompt`: Submits a user message to the active agent session.
     - `link`: Opens external URLs via system browser (`openExternalUrl`).
     - `notify`: Submits a system notification message.

4. **Rust Builtin MCP Backend** (`src-tauri/src/mcp/builtin/`)
   - Single unified backend executing all built-in MCP server modules (`ui`, `playbook`, `workspace`, `agent`, etc.).
   - Generates HTML UI resources returned as `MCPContent::Resource` with `mimeType: "text/html"` and `uri: "ui://..."`.

## Tool Naming Convention

All tools in LibrAgent (both built-in Rust servers and external MCP servers) follow the unified `server__tool` naming convention:

```typescript
// Format: ${serverName}__${toolName}
playbook__show_playbook;
ui__select_prompt;
planning__create_plan;
filesystem__read_file;
github__create_issue;
```

> [!NOTE]
> The legacy `builtin_` prefix (e.g., `builtin_ui__select_prompt`) has been deprecated in favor of the unified `{server}__{tool}` format across both Rust backend and frontend routing.

## Implementation Steps for Built-in MCP Servers (Rust Backend)

### 1. Define Server & Tools in Rust

Implement the `BuiltinMCPServer` trait in `src-tauri/src/mcp/builtin/<module>/`:

```rust
// src-tauri/src/mcp/builtin/example/mod.rs
use async_trait::async_trait;
use serde_json::{json, Value};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, MCPResource};

#[derive(Debug)]
pub struct ExampleServer;

#[async_trait]
impl BuiltinMCPServer for ExampleServer {
    fn name(&self) -> &str {
        "example"
    }

    fn description(&self) -> &str {
        "Example server producing UI resources"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![/* tool definitions */]
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "show_items" => self.handle_show_items(args).await,
            "select_item" => self.handle_select_item(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
```

### 2. Generate UI Resource Response

Return HTML wrapped in an `MCPContent::Resource` item:

```rust
impl ExampleServer {
    async fn handle_show_items(&self, _args: Value) -> Result<MCPResult, String> {
        let html_content = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8" />
    <style>
        .action-btn {
            padding: 8px 16px;
            border-radius: 4px;
            cursor: pointer;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="item">
            <span>Item A</span>
            <button class="action-btn select-btn" data-id="item-a" data-value="Value A">Select</button>
        </div>
    </div>

    <script>
        document.addEventListener('DOMContentLoaded', function() {
            document.querySelectorAll('.select-btn').forEach(function(btn) {
                btn.addEventListener('click', function(e) {
                    const id = e.target.dataset.id;
                    const value = e.target.dataset.value;

                    window.parent.postMessage({
                        type: 'ui-action',
                        action: {
                            tool: 'select_item',
                            params: { id: id, value: value }
                        }
                    }, '*');
                });
            });
        });
    </script>
</body>
</html>
"#;

        Ok(MCPResult {
            content: Some(vec![MCPContent::Resource {
                resource: MCPResource {
                    uri: "ui://example/items".to_string(),
                    mime_type: "text/html".to_string(),
                    text: Some(html_content.to_string()),
                    blob: None,
                },
            }]),
            is_error: Some(false),
            structured_content: None,
        })
    }
}
```

### 3. Register Server in Rust Registry

Register the server entry in:

1. `src-tauri/src/mcp/builtin/service_id.rs`
2. `src-tauri/src/mcp/service_proxy/factory.rs`

```rust
// src-tauri/src/mcp/service_proxy/factory.rs
match service_id {
    BuiltinServiceId::Example => Ok(Some(Box::new(
        crate::mcp::builtin::example::ExampleServer::new(),
    ))),
    // ...
}
```

## Best Practices

### ✅ DO

1. **Use DOMContentLoaded event**

   ```javascript
   document.addEventListener('DOMContentLoaded', function () {
     // Attach event listeners safely after DOM is parsed
   });
   ```

2. **Use querySelectorAll for multiple elements**

   ```javascript
   document.querySelectorAll('.action-btn').forEach(function (btn) {
     btn.addEventListener('click', handleClick);
   });
   ```

3. **Use data attributes for parameters**

   ```html
   <button data-id="123" data-value="example">Click</button>
   ```

4. **Send structured postMessage format**

   ```javascript
   window.parent.postMessage(
     {
       type: 'ui-action',
       action: {
         tool: 'select_item', // Base name or full server__tool name
         params: { key: 'value' },
       },
     },
     '*',
   );
   ```

5. **Utilize supported action types**
   - `tool`: Runs MCP tool call via Rust backend (`handleUserToolCall`) or Tauri command (`tauri:*`).
   - `intent` / `prompt`: Enqueues text message into active chat session.
   - `link`: Opens external URL safely.
   - `notify`: Shows system notification.

### ❌ DON'T

1. **Don't use event delegation blindly**

   ```javascript
   // ❌ Fragile and hard to debug inside iframe
   document.addEventListener('click', function (e) {
     if (e.target.classList.contains('btn')) { ... }
   });
   ```

2. **Don't use inline event handlers**

   ```html
   <!-- ❌ Avoid inline scripts -->
   <button onclick="handleClick()">Click</button>
   ```

3. **Don't use `builtin_` prefix**
   - Rely on unified `{server}__{tool}` format.

## Debugging

### Enable Logging

Check browser console logs or debug logs:

```typescript
// useUIActionHandler.ts
logger.info('UI Action Tool Call Received', {
  sessionId,
  result,
});
```

### Common Issues

1. **UI action not triggered**
   - Check: Is `DOMContentLoaded` attached?
   - Check: Is `postMessage` formatted correctly with `type: 'ui-action'`?
   - Check: Are data attributes populated?

2. **Tool not found or routing error**
   - Check: Is tool name format `{server}__{tool}`?
   - Check: Is server registered in Rust `service_id.rs` and `factory.rs`?

3. **Styling / Theme mismatch**
   - Check: `applyThemeToUiResource` injects host CSS custom variables (`--background`, `--foreground`, etc.) into iframe header. Use CSS variables in template for dark/light mode compatibility.

## Key Files & Reference Implementations

### Key Files

- Component Renderer: `src/features/agent/components/AgentMessageRenderer/components/ContentItemRenderer.tsx`
- Action Handler Hook: `src/features/agent/components/AgentMessageRenderer/hooks/useUIActionHandler.ts`
- Backend API Bridge: `src/lib/backend.ts` (`handleUserToolCall`, `executeUiTauriAction`)
- Rust Builtin MCP Servers: `src-tauri/src/mcp/builtin/`
  - `src-tauri/src/mcp/builtin/ui/` (UI interaction server)
  - `src-tauri/src/mcp/builtin/playbook/` (Playbook workflow UI resources)

### Reference Implementations in Rust

1. **UI Server** (`src-tauri/src/mcp/builtin/ui/mod.rs`)
   - Interactive prompt selections, confirmation dialogs, and progress views.

2. **Playbook Server** (`src-tauri/src/mcp/builtin/playbook/`)
   - Renders playbook lists and action buttons returning `ui://playbook/...` HTML resources.

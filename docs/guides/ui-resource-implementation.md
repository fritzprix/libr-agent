# UI Resource Implementation Guide

## Overview

This guide explains how to implement UI resources in Web MCP servers for LibrAgent. UI resources allow tools to return interactive HTML interfaces that can trigger tool calls through user actions.

## Architecture

### Component Flow

```text
UIResourceRenderer (iframe) 
  → postMessage 
    → MessageRenderer 
      → useUnifiedMCP 
        → BuiltInToolProvider 
          → Web MCP Server
```

### Key Components

1. **UIResourceRenderer** (`@mcp-ui/client`)
   - Renders HTML content in sandboxed iframe
   - Listens for button clicks and form submissions
   - Sends postMessage events to parent window

2. **MessageRenderer** (`src/components/MessageRenderer.tsx`)
   - Receives postMessage from iframe
   - Extracts tool name and parameters
   - Routes to appropriate backend (BuiltInWeb/External MCP)

3. **useUnifiedMCP** (`src/hooks/use-unified-mcp.ts`)
   - Unified interface for tool execution
   - Routes between External MCP, BuiltInWeb, BuiltInRust

4. **BuiltInToolProvider** (`src/features/tools/index.tsx`)
   - Executes builtin tools with `builtin_` prefix
   - Parses tool name format: `builtin_${serverName}__${toolName}`

## Tool Naming Convention

### Web MCP Tools (BuiltInWeb)

```typescript
// Format: builtin_${serverName}__${toolName}
builtin_playbook__show_playbook
builtin_ui__select_prompt
builtin_planning__create_plan
```

### External MCP Tools

```typescript
// Format: ${serverName}__${toolName}
filesystem__read_file
github__create_issue
```

## Implementation Steps

### 1. Create Handlebars Template

Create a `.hbs` file in your Web MCP server's templates directory:

```handlebars
<!-- templates/example.hbs -->
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <style>
    /* Add your styles */
    .action-btn {
      padding: 8px 16px;
      border: none;
      border-radius: 4px;
      cursor: pointer;
    }
  </style>
</head>
<body>
  <div class="container">
    {{#each items}}
    <div class="item">
      <span>{{this.name}}</span>
      <button 
        class="action-btn select-btn" 
        data-id="{{this.id}}"
        data-value="{{this.value}}">
        Select
      </button>
    </div>
    {{/each}}
  </div>

  <script>
    // ✅ CORRECT: Explicit event listeners
    document.addEventListener('DOMContentLoaded', function() {
      // Attach listeners to each button
      document.querySelectorAll('.select-btn').forEach(function(btn) {
        btn.addEventListener('click', function(e) {
          const id = e.target.dataset.id;
          const value = e.target.dataset.value;
          
          // Send postMessage to parent
          window.parent.postMessage({
            type: 'ui-action',
            action: {
              tool: 'select_item',
              params: {
                id: id,
                value: value
              }
            }
          }, '*');
        });
      });
    });

    // ❌ WRONG: Event delegation (fragile, hard to debug)
    // document.addEventListener('click', function(e) {
    //   if (e.target.classList.contains('select-btn')) {
    //     // This pattern is unreliable
    //   }
    // });
  </script>
</body>
</html>
```

### 2. Implement Tool Handler

```typescript
// web-mcp/modules/example-server/index.ts
import { Tool } from '@modelcontextprotocol/sdk/types.js';
import Handlebars from 'handlebars';

const exampleTemplate = `<!-- your template content -->`;

class ExampleServer {
  async handleToolCall(toolName: string, params: unknown) {
    switch (toolName) {
      case 'show_items':
        return this.showItems(params);
      case 'select_item':
        return this.selectItem(params);
      default:
        throw new Error(`Unknown tool: ${toolName}`);
    }
  }

  private async showItems(params: unknown) {
    const items = await this.fetchItems();
    
    const template = Handlebars.compile(exampleTemplate);
    const html = template({ items });

    return {
      content: [
        {
          type: 'resource' as const,
          resource: {
            uri: 'ui://example/items',
            mimeType: 'text/html',
            text: html
          }
        }
      ]
    };
  }

  private async selectItem(params: { id: string; value: string }) {
    // Handle selection
    return {
      content: [
        {
          type: 'text' as const,
          text: `Selected item: ${params.value}`
        }
      ]
    };
  }
}
```

### 3. Register Server

```typescript
// src/app/App.tsx
import { WebMCPProvider } from '@/context/WebMCPContext';

function App() {
  return (
    <WebMCPProvider
      servers={[
        {
          name: 'example',
          path: '/src/lib/web-mcp/modules/example-server/worker.ts',
          enabled: true,
        },
        // ... other servers
      ]}
    >
      {/* app content */}
    </WebMCPProvider>
  );
}
```

### 4. Integrate with BuiltInToolProvider

The server is automatically registered through `WebMCPServiceRegistry`:

```typescript
// src/lib/web-mcp/registry.ts
export class WebMCPServiceRegistry {
  async registerServer(
    name: string,
    getServerProxy: () => WebMCPServerProxy | null
  ): Promise<void> {
    const service: BuiltInService = {
      name: `builtin_${name}`,
      description: `Web MCP Server: ${name}`,
      
      async listTools() {
        const proxy = getServerProxy();
        if (!proxy) return [];
        return proxy.listTools();
      },
      
      async executeTool(toolName: string, params: ToolParams) {
        const proxy = getServerProxy();
        if (!proxy) throw new Error('Server not available');
        return proxy.callTool(toolName, params);
      }
    };

    this.services.set(name, service);
  }
}
```

## Best Practices

### ✅ DO

1. **Use DOMContentLoaded event**

   ```javascript
   document.addEventListener('DOMContentLoaded', function() {
     // Attach listeners here
   });
   ```

2. **Use querySelectorAll for multiple elements**

   ```javascript
   document.querySelectorAll('.action-btn').forEach(function(btn) {
     btn.addEventListener('click', handleClick);
   });
   ```

3. **Use data attributes for parameters**

   ```html
   <button data-id="123" data-value="example">Click</button>
   ```

4. **Send structured postMessage**

   ```javascript
   window.parent.postMessage({
     type: 'ui-action',
     action: {
       tool: 'tool_name',
       params: { key: 'value' }
     }
   }, '*');
   ```

5. **Match ui-tools pattern**
   - Reference: `/src/lib/web-mcp/modules/ui-tools/templates/select-prompt.hbs`

### ❌ DON'T

1. **Don't use event delegation**

   ```javascript
   // ❌ Fragile and hard to debug
   document.addEventListener('click', function(e) {
     if (e.target.classList.contains('btn')) { }
   });
   ```

2. **Don't minify template JavaScript**
   - Keep code readable for debugging
   - Use proper formatting and comments

3. **Don't use inline event handlers**

   ```html
   <!-- ❌ Avoid this -->
   <button onclick="handleClick()">Click</button>
   ```

4. **Don't assume DOM is ready**

   ```javascript
   // ❌ May fail if script runs before DOM loads
   document.querySelector('.btn').addEventListener('click', handler);
   ```

## Debugging

### Enable Logging

Temporarily add logs to diagnose issues:

```typescript
// MessageRenderer.tsx
const handleUIAction = (toolName: string, params: unknown) => {
  console.log('🎨 UI Action:', { toolName, params, serviceInfo });
  
  if (serviceInfo.backendType === 'BuiltInWeb') {
    console.log('🌐 Web MCP tool:', finalToolName);
  }
  
  executeToolCall(finalToolName, params);
};
```

### Common Issues

1. **UI action not triggered**
   - Check: DOMContentLoaded event attached?
   - Check: Event listeners on correct elements?
   - Check: postMessage format correct?

2. **Tool not found**
   - Check: Tool name matches handler switch case?
   - Check: Server registered in App.tsx?
   - Check: `builtin_` prefix added for Web MCP?

3. **Parameters not passed**
   - Check: data attributes on HTML elements?
   - Check: postMessage includes params object?
   - Check: Handler receives correct params?

## Testing Checklist

- [ ] Template renders in iframe without errors
- [ ] Buttons are clickable and responsive
- [ ] postMessage sent to parent window
- [ ] MessageRenderer receives and routes action
- [ ] Tool handler executes with correct params
- [ ] Response rendered in chat interface
- [ ] Works in both development and production builds

## Reference Implementations

### Working Examples

1. **ui-tools** (`/src/lib/web-mcp/modules/ui-tools/`)
   - select-prompt.hbs: Multi-option selection
   - Uses explicit addEventListener pattern
   - Clean, readable JavaScript

2. **playbook-store** (`/src/lib/web-mcp/modules/playbook-store/`)
   - playbooks.hbs: Playbook listing with actions
   - Refactored to match ui-tools pattern
   - Handles Select/Delete/Navigate buttons

### Key Files

- MessageRenderer: `/src/components/MessageRenderer.tsx`
- useUnifiedMCP: `/src/hooks/use-unified-mcp.ts`
- BuiltInToolProvider: `/src/features/tools/index.tsx`
- WebMCPServiceRegistry: `/src/lib/web-mcp/registry.ts`

## Summary

1. Create Handlebars template with explicit event listeners
2. Use DOMContentLoaded and querySelectorAll pattern
3. Send postMessage with `type: 'ui-action'` and tool/params
4. Implement tool handler with switch case for each action
5. Register server in App.tsx WebMCPProvider
6. Test thoroughly in both dev and production

For questions or issues, refer to working examples in ui-tools and playbook-store modules.

/**
 * 🎨 MCP-UI Demo Tool
 *
 * This file demonstrates how to create tools that return UIResource objects
 * for testing the MCP-UI integration in the chat interface.
 */

import { UIResource } from '@/models/chat';
import { MCPResponse, MCPResourceContent } from '@/lib/mcp-types';

// Sample HTML content for testing
const SAMPLE_HTML = `
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>MCP-UI Demo</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
            margin: 0;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
        }
        .demo-card {
            background: rgba(255, 255, 255, 0.1);
            border-radius: 12px;
            padding: 30px;
            backdrop-filter: blur(10px);
            text-align: center;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
            max-width: 400px;
        }
        .demo-button {
            background: #4CAF50;
            border: none;
            color: white;
            padding: 12px 24px;
            text-align: center;
            text-decoration: none;
            display: inline-block;
            font-size: 16px;
            margin: 10px 2px;
            cursor: pointer;
            border-radius: 8px;
            transition: background-color 0.3s;
        }
        .demo-button:hover {
            background: #45a049;
        }
        .demo-button.secondary {
            background: #2196F3;
        }
        .demo-button.secondary:hover {
            background: #1976D2;
        }
        .timestamp {
            font-size: 12px;
            opacity: 0.8;
            margin-top: 15px;
        }
    </style>
</head>
<body>
    <div class="demo-card">
        <h1>🎨 MCP-UI Demo</h1>
        <p>This is a sample UI component rendered from an MCP tool response!</p>

        <button class="demo-button" onclick="sendToolAction()">
            🔧 Call Another Tool
        </button>

        <button class="demo-button secondary" onclick="sendPrompt()">
            💬 Send Prompt
        </button>

        <button class="demo-button secondary" onclick="showNotification()">
            🔔 Show Notification
        </button>

        <div class="timestamp">
            Generated at: ${new Date().toLocaleString()}
        </div>
    </div>

    <script>
        function sendToolAction() {
            window.parent.postMessage({
                type: 'tool',
                payload: {
                    toolName: 'demo__greet',
                    params: {
                        name: 'MCP-UI User',
                        from: 'ui-component'
                    }
                }
            }, '*');
        }

        function sendPrompt() {
            window.parent.postMessage({
                type: 'prompt',
                payload: {
                    prompt: 'What other UI components can you create with MCP-UI?'
                }
            }, '*');
        }

        function showNotification() {
            window.parent.postMessage({
                type: 'notify',
                payload: {
                    message: 'Hello from the UI component! 👋'
                }
            }, '*');
        }

        // Send initial notification when loaded
        window.addEventListener('load', function() {
            setTimeout(() => {
                window.parent.postMessage({
                    type: 'notify',
                    payload: {
                        message: 'MCP-UI component loaded successfully!'
                    }
                }, '*');
            }, 500);
        });
    </script>
</body>
</html>
`;

// Sample Remote DOM script
const SAMPLE_REMOTE_DOM_SCRIPT = `
// Remote DOM script for MCP-UI
const container = document.createElement('ui-container');
container.setAttribute('style', 'padding: 20px; background: #f5f5f5; border-radius: 8px;');

const heading = document.createElement('ui-heading');
heading.setAttribute('level', '2');
heading.textContent = '🚀 Remote DOM Demo';

const text = document.createElement('ui-text');
text.textContent = 'This UI is rendered using Remote DOM technology!';

const button = document.createElement('ui-button');
button.setAttribute('variant', 'primary');
button.setAttribute('label', 'Execute Tool');
button.addEventListener('press', () => {
    window.parent.postMessage({
        type: 'tool',
        payload: {
            toolName: 'demo__getCurrentTime',
            params: {}
        }
    }, '*');
});

container.appendChild(heading);
container.appendChild(text);
container.appendChild(button);

root.appendChild(container);
`;

/**
 * Demo tool that returns HTML UIResource
 */
export function createHtmlDemo(): MCPResponse {
  const uiResource: UIResource = {
    uri: 'ui://demo/html-component',
    mimeType: 'text/html',
    text: SAMPLE_HTML,
  };

  return {
    jsonrpc: '2.0',
    id: `demo-html-${Date.now()}`,
    result: {
      content: [
        {
          type: 'resource',
          resource: uiResource,
        } as MCPResourceContent,
      ],
    },
  };
}

/**
 * Demo tool that returns external URL UIResource
 */
export function createUrlDemo(): MCPResponse {
  const uiResource: UIResource = {
    uri: 'ui://demo/external-url',
    mimeType: 'text/uri-list',
    text: 'https://modelcontextprotocol.io/introduction\n# MCP Introduction Page',
  };

  return {
    jsonrpc: '2.0',
    id: `demo-url-${Date.now()}`,
    result: {
      content: [
        {
          type: 'resource',
          resource: uiResource,
        } as MCPResourceContent,
      ],
    },
  };
}

/**
 * Demo tool that returns Remote DOM UIResource
 */
export function createRemoteDomDemo(): MCPResponse {
  const uiResource: UIResource = {
    uri: 'ui://demo/remote-dom-component',
    mimeType: 'application/vnd.mcp-ui.remote-dom',
    text: SAMPLE_REMOTE_DOM_SCRIPT,
  };

  return {
    jsonrpc: '2.0',
    id: `demo-remote-dom-${Date.now()}`,
    result: {
      content: [
        {
          type: 'resource',
          resource: uiResource,
        } as MCPResourceContent,
      ],
    },
  };
}

/**
 * Demo tool that returns mixed content (text + UIResource)
 */
export function createMixedDemo(): MCPResponse {
  const uiResource: UIResource = {
    uri: 'ui://demo/mixed-content',
    mimeType: 'text/html',
    text: `
            <div style="padding: 20px; background: #e8f5e8; border-radius: 8px; font-family: sans-serif;">
                <h3 style="color: #2e7d32; margin-top: 0;">✅ Mixed Content Demo</h3>
                <p>This demonstrates a tool response that contains both text and UI resources.</p>
                <button onclick="window.parent.postMessage({type: 'notify', payload: {message: 'Mixed content interaction!'}}, '*')"
                        style="background: #4caf50; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer;">
                    Click Me!
                </button>
            </div>
        `,
  };

  return {
    jsonrpc: '2.0',
    id: `demo-mixed-${Date.now()}`,
    result: {
      content: [
        {
          type: 'text',
          text: 'Here is some text content along with an interactive UI component:',
        },
        {
          type: 'resource',
          resource: uiResource,
        } as MCPResourceContent,
      ],
    },
  };
}

/**
 * Demo tool that returns multiple UIResources
 */
export function createMultiResourceDemo(): MCPResponse {
  const htmlResource: UIResource = {
    uri: 'ui://demo/multi-html',
    mimeType: 'text/html',
    text: `
            <div style="padding: 15px; background: #fff3cd; border: 1px solid #ffeaa7; border-radius: 8px; margin-bottom: 10px;">
                <h4 style="margin: 0 0 10px 0; color: #856404;">📊 Resource #1 (HTML)</h4>
                <p style="margin: 0;">This is the first UI resource in a multi-resource response.</p>
            </div>
        `,
  };

  const urlResource: UIResource = {
    uri: 'ui://demo/multi-url',
    mimeType: 'text/uri-list',
    text: 'https://github.com/modelcontextprotocol/specification',
  };

  return {
    jsonrpc: '2.0',
    id: `demo-multi-${Date.now()}`,
    result: {
      content: [
        {
          type: 'text',
          text: 'This response contains multiple UI resources:',
        },
        {
          type: 'resource',
          resource: htmlResource,
        } as MCPResourceContent,
        {
          type: 'text',
          text: 'And here is an external resource:',
        },
        {
          type: 'resource',
          resource: urlResource,
        } as MCPResourceContent,
      ],
    },
  };
}

/**
 * Simple greeting tool for testing tool calls from UI
 */
export function createGreetingResponse(
  name: string,
  from?: string,
): MCPResponse {
  return {
    jsonrpc: '2.0',
    id: `greeting-${Date.now()}`,
    result: {
      content: [
        {
          type: 'text',
          text: `Hello, ${name}! 👋${from ? ` (Called from: ${from})` : ''}`,
        },
      ],
    },
  };
}

/**
 * Current time tool for testing
 */
export function createCurrentTimeResponse(): MCPResponse {
  return {
    jsonrpc: '2.0',
    id: `time-${Date.now()}`,
    result: {
      content: [
        {
          type: 'text',
          text: `Current time: ${new Date().toLocaleString()}`,
        },
      ],
    },
  };
}

// Export all demo functions for use in web MCP tools or testing
export const mcpUiDemoTools = {
  createHtmlDemo,
  createUrlDemo,
  createRemoteDomDemo,
  createMixedDemo,
  createMultiResourceDemo,
  createGreetingResponse,
  createCurrentTimeResponse,
};

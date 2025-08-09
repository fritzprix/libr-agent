/**
 * 🌐 Web Worker MCP Server Implementation
 *
 * This worker runs MCP servers in a web worker environment,
 * providing MCP-compatible functionality without Node.js/Python dependencies.
 */

import {
  WebMCPServer,
  WebMCPMessage,
  WebMCPResponse,
  MCPTool,
} from '../mcp-types';

// Cache for loaded MCP servers
const mcpServers = new Map<string, WebMCPServer>();

/**
 * Dynamically load an MCP server module
 */
async function loadMCPServer(serverName: string): Promise<WebMCPServer> {
  if (mcpServers.has(serverName)) {
    return mcpServers.get(serverName)!;
  }

  try {
    // Dynamic import of MCP server modules
    const mod = await import(`./modules/${serverName}`);
    const server = mod.default as WebMCPServer;

    // Validate server structure
    if (
      !server.name ||
      !server.tools ||
      typeof server.callTool !== 'function'
    ) {
      throw new Error(`Invalid MCP server module: ${serverName}`);
    }

    mcpServers.set(serverName, server);
    console.log(
      `[WebMCP Worker] Loaded server: ${serverName} with ${server.tools.length} tools`,
    );
    return server;
  } catch (error) {
    console.error(
      `[WebMCP Worker] Failed to load server ${serverName}:`,
      error,
    );
    throw new Error(`Failed to load MCP server: ${serverName}`);
  }
}

/**
 * Handle MCP message and return appropriate response
 */
async function handleMCPMessage(
  message: WebMCPMessage,
): Promise<WebMCPResponse> {
  const { id, type, serverName, toolName, args } = message;

  try {
    switch (type) {
      case 'ping':
        return { id, result: 'pong' };

      case 'loadServer': {
        if (!serverName) {
          throw new Error('Server name is required for loadServer');
        }
        const loadedServer = await loadMCPServer(serverName);
        return {
          id,
          result: {
            name: loadedServer.name,
            description: loadedServer.description,
            version: loadedServer.version,
            toolCount: loadedServer.tools.length,
          },
        };
      }

      case 'listTools': {
        if (!serverName) {
          // Return tools from all loaded servers
          const allTools: MCPTool[] = [];
          for (const [name, server] of mcpServers.entries()) {
            // Prefix tool names with server name for uniqueness
            const prefixedTools = server.tools.map((tool) => ({
              ...tool,
              name: `${name}__${tool.name}`,
            }));
            allTools.push(...prefixedTools);
          }
          return { id, result: allTools };
        } else {
          // Return tools from specific server
          const server = await loadMCPServer(serverName);
          const prefixedTools = server.tools.map((tool) => ({
            ...tool,
            name: `${serverName}__${tool.name}`,
          }));
          return { id, result: prefixedTools };
        }
      }

      case 'callTool': {
        if (!serverName || !toolName) {
          throw new Error(
            'Server name and tool name are required for callTool',
          );
        }

        const server = await loadMCPServer(serverName);
        const result = await server.callTool(toolName, args);
        return { id, result };
      }

      default:
        throw new Error(`Unknown MCP message type: ${type}`);
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error(`[WebMCP Worker] Error handling message:`, {
      message,
      error: errorMessage,
    });
    return { id, error: errorMessage };
  }
}

/**
 * Worker message handler
 */
self.onmessage = async (event: MessageEvent<WebMCPMessage>) => {
  try {
    const response = await handleMCPMessage(event.data);
    self.postMessage(response);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error(`[WebMCP Worker] Unhandled error:`, error);

    // Send error response
    const errorResponse: WebMCPResponse = {
      id: event.data?.id || 'unknown',
      error: `Worker error: ${errorMessage}`,
    };
    self.postMessage(errorResponse);
  }
};

/**
 * Worker error handler
 */
self.onerror = (error) => {
  console.error('[WebMCP Worker] Worker error:', error);
};

/**
 * Worker unhandled rejection handler
 */
self.onunhandledrejection = (event) => {
  console.error('[WebMCP Worker] Unhandled rejection:', event.reason);
  event.preventDefault();
};

console.log('[WebMCP Worker] Web Worker MCP server initialized');

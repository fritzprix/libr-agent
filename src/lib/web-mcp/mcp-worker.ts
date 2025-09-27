/**
 * @file Web Worker implementation for running MCP (Model Context Protocol) servers.
 *
 * This script runs in a separate thread as a Web Worker, providing an isolated
 * environment for executing MCP-compatible servers and tools without blocking the
 * main UI thread. It communicates with the main application using `postMessage`.
 *
 * It uses static imports for server modules to ensure compatibility with bundlers
 * like Vite and to provide better type safety.
 */

import type {
  WebMCPServer,
  WebMCPMessage,
  MCPResponse,
  MCPTool,
} from '../mcp-types';

// Static imports for MCP server modules to avoid Vite dynamic import warnings
// This approach provides better bundling compatibility and type safety
import planningServer from './modules/planning-server';

/**
 * A simple logger for the worker context, as the main logger is not available here.
 * @internal
 */
const log = {
  debug: (message: string, data?: unknown) => {
    console.log(`[WebMCP Worker][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[WebMCP Worker][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[WebMCP Worker][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[WebMCP Worker][ERROR] ${message}`, data || '');
  },
};

// Static module registry - using direct imports instead of dynamic imports
// This eliminates Vite bundling warnings and provides better type safety
const MODULE_REGISTRY = [
  { key: 'planning', module: planningServer },
  // Future modules can be added here with static imports
] as const;

// Initialize server instances directly with static modules
const serverInstances = new Map<string, WebMCPServer | null>(
  MODULE_REGISTRY.map(({ key, module }) => [key, module]),
);

/**
 * Logs the status of statically imported servers. Since modules are imported
 * statically at the top of the file, this function primarily serves to confirm
 * that the modules have been loaded into the registry correctly.
 * @internal
 */
async function loadServers(): Promise<void> {
  try {
    log.debug('MCP servers already loaded via static imports');

    // Log loaded servers for debugging
    MODULE_REGISTRY.forEach(({ key, module }) => {
      if (module) {
        log.debug(`${key} server loaded statically`);
      } else {
        log.warn(`${key} server module is null`);
      }
    });

    log.info('Static server loading completed');
  } catch (error) {
    log.error('Critical error during static server loading', error);
  }
}

/**
 * Gets the registry of server instances.
 * @returns A map of server names to server instances.
 * @internal
 */
const getServerRegistry = (): Map<string, WebMCPServer | null> => {
  return serverInstances;
};

// Cache for loaded MCP servers
const mcpServers = new Map<string, WebMCPServer>();

/**
 * Retrieves a loaded MCP server instance from the cache or loads it from the registry.
 * @param serverName The name of the server to load.
 * @returns A promise that resolves to the `WebMCPServer` instance.
 * @throws An error if the server is not found or is invalid.
 * @internal
 */
async function loadMCPServer(serverName: string): Promise<WebMCPServer> {
  if (mcpServers.has(serverName)) {
    return mcpServers.get(serverName)!;
  }

  try {
    // Servers are already loaded via static imports, no need to load dynamically
    // This check is kept for safety but should always pass with static imports
    const allServersLoaded = Array.from(serverInstances.values()).every(
      (s) => s !== null,
    );
    if (!allServersLoaded) {
      log.warn('Some servers are not loaded, attempting to reload');
      await loadServers();
    }

    // Get server from registry
    const serverRegistry = getServerRegistry();
    const server = serverRegistry.get(serverName);

    if (!server) {
      const availableServers = Array.from(serverRegistry.keys());
      throw new Error(
        `Unknown MCP server: ${serverName}. Available: ${availableServers.join(', ')}`,
      );
    }

    // Validate server structure
    if (
      !server.name ||
      !server.tools ||
      typeof server.callTool !== 'function'
    ) {
      throw new Error(`Invalid MCP server module: ${serverName}`);
    }

    mcpServers.set(serverName, server);
    log.info('Server loaded', { serverName, toolCount: server.tools.length });
    return server;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    log.error('Failed to load server', { serverName, error: errorMessage });
    throw new Error(
      `Failed to load MCP server: ${serverName} - ${errorMessage}`,
    );
  }
}

/**
 * Handles an incoming `WebMCPMessage` from the main thread, routes it to the
 * appropriate action (e.g., ping, loadServer, callTool), and returns a response.
 * @param message The message from the main thread.
 * @returns A promise that resolves to an `MCPResponse` to be sent back to the main thread.
 * @internal
 */
async function handleMCPMessage(
  message: WebMCPMessage,
): Promise<MCPResponse<unknown>> {
  const { id, type, serverName, toolName, args } = message;

  log.debug('Handling MCP message', {
    id,
    type,
    serverName,
    toolName,
    hasArgs: !!args,
  });

  try {
    switch (type) {
      case 'ping':
        log.debug('Handling ping request');
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: 'pong' }],
            structuredContent: 'pong',
          },
        };

      case 'loadServer': {
        if (!serverName) {
          throw new Error('Server name is required for loadServer');
        }

        const loadedServer = await loadMCPServer(serverName);
        const serverInfo = {
          name: loadedServer.name,
          description: loadedServer.description,
          version: loadedServer.version,
          toolCount: loadedServer.tools.length,
        };
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [
              {
                type: 'text',
                text: JSON.stringify(serverInfo, null, 2),
              },
            ],
            structuredContent: serverInfo,
          },
        };
      }

      case 'listTools': {
        if (!serverName) {
          // Return tools from all loaded servers
          const allTools: MCPTool[] = [];
          for (const server of mcpServers.values()) {
            allTools.push(...server.tools);
          }
          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(allTools),
                },
              ],
              structuredContent: allTools,
            },
          };
        } else {
          // Return tools from specific server
          const server = await loadMCPServer(serverName);
          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(server.tools),
                },
              ],
              structuredContent: server.tools,
            },
          };
        }
      }

      case 'callTool': {
        if (!serverName || !toolName) {
          throw new Error(
            'Server name and tool name are required for callTool',
          );
        }

        const server = await loadMCPServer(serverName);

        try {
          const result = await server.callTool(toolName, args);

          // Log the detailed tool result for debugging/UI inspection
          log.info('callTool result', { id, serverName, toolName, result });

          // Return MCPResponse directly since callTool now returns MCPResponse
          // but update the id to match the request
          const response = {
            ...result,
            id,
          };

          return response;
        } catch (toolError) {
          log.error('Tool call failed', {
            id,
            serverName,
            toolName,
            error:
              toolError instanceof Error
                ? toolError.message
                : String(toolError),
          });
          return {
            jsonrpc: '2.0',
            id,
            error: {
              code: -32603,
              message:
                toolError instanceof Error
                  ? toolError.message
                  : String(toolError),
            },
          };
        }
      }

      case 'getServiceContext': {
        if (!serverName) {
          throw new Error('Server name is required for getServiceContext');
        }
        const server = await loadMCPServer(serverName);
        if (server.getServiceContext) {
          const context = await server.getServiceContext();
          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: context,
                },
              ],
              structuredContent: context,
            },
          };
        }
        // Fallback for servers without getServiceContext
        const context = `# MCP Server Context\nServer: ${serverName}\nStatus: Connected\nAvailable Tools: ${server.tools.length} tools`;
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [
              {
                type: 'text',
                text: context,
              },
            ],
            structuredContent: context,
          },
        };
      }

      default: {
        throw new Error(`Unknown MCP message type: ${type}`);
      }
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);

    log.error('Error handling MCP message', {
      id,
      type,
      serverName,
      toolName,
      error: errorMessage,
    });

    return {
      jsonrpc: '2.0',
      id,
      error: {
        code: -32603,
        message: errorMessage,
      },
    };
  }
}

/**
 * The main message handler for the worker. It listens for messages from the main
 * thread, passes them to `handleMCPMessage`, and posts the response back.
 */
self.onmessage = async (event: MessageEvent<WebMCPMessage>) => {
  const messageId = event.data?.id || 'unknown';

  try {
    const response = await handleMCPMessage(event.data);
    self.postMessage(response);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);

    log.error('Worker message handler error', {
      id: messageId,
      error: errorMessage,
    });

    const errorResponse: MCPResponse<unknown> = {
      jsonrpc: '2.0',
      id: messageId,
      error: {
        code: -32603,
        message: `Worker error: ${errorMessage}`,
      },
    };

    self.postMessage(errorResponse);
  }
};

/**
 * The global error handler for the worker.
 */
self.onerror = (error) => {
  log.error('Worker error', { error: String(error) });
};

/**
 * The handler for unhandled promise rejections in the worker.
 */
self.onunhandledrejection = (event) => {
  log.error('Unhandled rejection', { reason: String(event.reason) });
  event.preventDefault();
};

// Initialize worker and load servers
log.info('Initializing WebMCP worker');
loadServers()
  .then(() => {
    log.info('WebMCP worker ready');
  })
  .catch((error) => {
    log.error('Worker initialization failed', { error: String(error) });
  });

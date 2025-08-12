/**
 * 🌐 Web Worker MCP Server Implementation
 *
 * This worker runs MCP servers in a web worker environment,
 * providing MCP-compatible functionality without Node.js/Python dependencies.
 *
 * Designed for Vite's ?worker import system
 */

import type {
  WebMCPServer,
  WebMCPMessage,
  MCPResponse,
  MCPTool,
} from '../mcp-types';

// Add console logging for debugging since we can't use our logger in worker context
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

// Static imports for better bundling with Vite
let calculatorServer: WebMCPServer | null = null;
let filesystemServer: WebMCPServer | null = null;
let contentStoreServer: WebMCPServer | null = null;

// Dynamic imports with error handling
async function loadServers(): Promise<void> {
  try {
    log.debug('Loading MCP servers');

    // Load all servers
    const [calculatorModule, filesystemModule, contentStoreModule] =
      await Promise.allSettled([
        import('./modules/calculator'),
        import('./modules/filesystem'),
        import('./modules/content-store'),
      ]);

    if (calculatorModule.status === 'fulfilled') {
      calculatorServer = calculatorModule.value.default;
      log.debug('Calculator server loaded');
    } else {
      log.error('Failed to load calculator server', calculatorModule.reason);
    }

    if (filesystemModule.status === 'fulfilled') {
      filesystemServer = filesystemModule.value.default;
      log.debug('Filesystem server loaded');
    } else {
      log.error('Failed to load filesystem server', filesystemModule.reason);
    }

    if (contentStoreModule.status === 'fulfilled') {
      contentStoreServer = contentStoreModule.value.default;
      log.debug('Content-store server loaded');
    } else {
      log.error(
        'Failed to load content-store server',
        contentStoreModule.reason,
      );
    }

    log.info('Server loading completed');
  } catch (error) {
    log.error('Critical error during server loading', error);
  }
}

// Server registry with dynamic loading
const getServerRegistry = (): Map<string, WebMCPServer | null> => {
  return new Map([
    ['calculator', calculatorServer],
    ['filesystem', filesystemServer],
    ['content-store', contentStoreServer],
  ]);
};

// Cache for loaded MCP servers
const mcpServers = new Map<string, WebMCPServer>();

/**
 * Load an MCP server from the registry
 */
async function loadMCPServer(serverName: string): Promise<WebMCPServer> {
  if (mcpServers.has(serverName)) {
    return mcpServers.get(serverName)!;
  }

  try {
    // Ensure servers are loaded
    if (!calculatorServer && !filesystemServer && !contentStoreServer) {
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
 * Handle MCP message and return appropriate response
 */
async function handleMCPMessage(message: WebMCPMessage): Promise<MCPResponse> {
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
            content: [
              {
                type: 'text',
                text: 'pong',
              },
            ],
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
          },
        };
      }

      case 'listTools': {
        if (!serverName) {
          // Return tools from all loaded servers
          const allTools: MCPTool[] = [];
          for (const [name, server] of mcpServers.entries()) {
            const prefixedTools = server.tools.map((tool) => ({
              ...tool,
              name: `${name}__${tool.name}`,
            }));
            allTools.push(...prefixedTools);
          }
          return {
            jsonrpc: '2.0',
            id,
            result: {
              structuredContent: { tools: allTools },
            },
          };
        } else {
          // Return tools from specific server
          const server = await loadMCPServer(serverName);
          const prefixedTools = server.tools.map((tool) => ({
            ...tool,
            name: `${serverName}__${tool.name}`,
          }));
          return {
            jsonrpc: '2.0',
            id,
            result: {
              structuredContent: { tools: prefixedTools },
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

          log.debug('Tool call completed', {
            id,
            serverName,
            toolName,
          });

          // Tool 결과를 표준 MCPResponse 형식으로 변환
          const textContent =
            typeof result === 'string'
              ? result
              : JSON.stringify(result, null, 2);

          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: textContent,
                },
              ],
            },
          };
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
          throw toolError;
        }
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
        code: -32603, // Internal error
        message: errorMessage,
      },
    };
  }
}

/**
 * Worker message handler
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

    const errorResponse: MCPResponse = {
      jsonrpc: '2.0',
      id: messageId,
      error: {
        code: -32603, // Internal error
        message: `Worker error: ${errorMessage}`,
      },
    };

    self.postMessage(errorResponse);
  }
};

/**
 * Worker error handler
 */
self.onerror = (error) => {
  log.error('Worker error', { error: String(error) });
};

/**
 * Worker unhandled rejection handler
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

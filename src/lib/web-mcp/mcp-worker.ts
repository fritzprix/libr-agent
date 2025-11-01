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
import { ServiceContext, ServiceContextOptions } from '../../features/tools';

// Static imports for MCP server modules to avoid Vite dynamic import warnings
// This approach provides better bundling compatibility and type safety
import planningServer from './modules/planning-server/index.ts';
// Import from the new playbook-store submodule (index.ts)
import playbookStore from './modules/playbook-store/index.ts';
import uiTools from './modules/ui-tools/index.ts';
import bootstrapServer from './modules/bootstrap-server/index.ts';

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
  { key: 'playbook', module: playbookStore },
  { key: 'ui', module: uiTools },
  { key: 'bootstrap', module: bootstrapServer },
  // Future modules can be added here with static imports
] as const;

// Initialize server instances directly with static modules
const serverInstances = new Map<string, WebMCPServer>(
  MODULE_REGISTRY.map(({ key, module }) => [key, module]),
);

/**
 * Retrieves an MCP server instance from the static registry.
 * Since servers are loaded via static imports, this is a simple lookup.
 * @param serverName The name of the server to retrieve.
 * @returns The WebMCPServer instance.
 * @throws An error if the server is not found.
 * @internal
 */
function getMCPServer(serverName: string): WebMCPServer {
  const server = serverInstances.get(serverName);
  if (!server) {
    const availableServers = Array.from(serverInstances.keys());
    throw new Error(
      `Unknown MCP server: ${serverName}. Available: ${availableServers.join(', ')}`,
    );
  }
  return server;
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

        const loadedServer = getMCPServer(serverName);
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
          for (const server of serverInstances.values()) {
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
          const server = getMCPServer(serverName);
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

        const server = getMCPServer(serverName);

        try {
          const result = await server.callTool(toolName, args);

          // Log tool call completion (without full result for performance)
          log.debug('Tool call completed', { id, serverName, toolName });

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
        const server = getMCPServer(serverName);
        if (server.getServiceContext) {
          const context = await server.getServiceContext(
            args as ServiceContextOptions | undefined,
          );
          // context가 ServiceContext인 경우 그대로 반환
          if (
            typeof context === 'object' &&
            context !== null &&
            'contextPrompt' in context &&
            'structuredState' in context
          ) {
            const serviceContext = context as ServiceContext<unknown>;
            return {
              jsonrpc: '2.0',
              id,
              result: {
                content: [
                  {
                    type: 'text',
                    text: serviceContext.contextPrompt,
                  },
                ],
                structuredContent: serviceContext,
              },
            };
          }
          // 레거시 string 반환의 경우 ServiceContext로 변환
          const contextString = typeof context === 'string' ? context : '';
          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: contextString,
                },
              ],
              structuredContent: {
                contextPrompt: contextString,
                structuredState: undefined,
              },
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
            structuredContent: {
              contextPrompt: context,
              structuredState: undefined,
            },
          },
        };
      }
      case 'switchContext': {
        if (!serverName) {
          throw new Error('Server name is required for switchContext');
        }
        const server = getMCPServer(serverName);
        if (server.switchContext) {
          await server.switchContext((args as ServiceContextOptions) || {});
          return {
            jsonrpc: '2.0',
            id,
            result: {
              content: [
                {
                  type: 'text',
                  text: 'Context switched successfully',
                },
              ],
              structuredContent: { success: true },
            },
          };
        }
        // Fallback for servers without setContext
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [
              {
                type: 'text',
                text: 'Server does not support context switching',
              },
            ],
            structuredContent: { success: false },
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

// Initialize worker
log.info('Initializing WebMCP worker');
log.info('WebMCP worker ready - servers loaded via static imports');

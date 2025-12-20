/**
 * @file Web Worker implementation for running MCP (Model Context Protocol) servers.
 *
 * This script runs in a separate thread as a Web Worker, providing an isolated
 * environment for executing MCP-compatible servers and tools without blocking the
 * main UI thread. It communicates with the main application using `postMessage`.
 *
 * ## Server Structure
 *
 * Each Web MCP server is a WebMCPServer instance with flat properties:
 * - `name`: Internal server identifier
 * - `displayName`: Human-readable name shown in UI
 * - `description`: Brief description of server capabilities
 * - `category`: UI grouping category (automation, storage, planning, execution)
 * - `tools`: Array of MCP tools provided by the server
 * - `callTool`: Function to execute tools
 *
 * ## Module Structure Example
 *
 * Each MCP server module exports a configured WebMCPServer instance:
 * ```typescript
 * // modules/planning-server/server.ts
 * const planningServer: WebMCPServer = {
 *   name: 'planning',
 *   displayName: 'Task Planning',
 *   description: 'Goal setting, task planning',
 *   category: 'planning',
 *   tools: [...],
 *   callTool: async (name, args) => { ... }
 * };
 * export default planningServer;
 *
 * // modules/planning-server/index.ts
 * export { default } from './server.ts';
 * ```
 *
 * ## Adding New Servers
 *
 * To add a new Web MCP server:
 * 1. Create the module directory with index.ts, server.ts, tools.ts
 * 2. Define your server with all required properties (name, displayName, description, tools, callTool)
 * 3. Add default import: `import newServer from './modules/new-server/index.ts';`
 * 4. Register in MODULE_REGISTRY: `{ key: 'new_server', server: newServer }`
 * 5. The server metadata will be automatically extracted and displayed in the UI
 */

import type {
  WebMCPServer,
  WebMCPMessage,
  MCPResponse,
  MCPTool,
} from '../mcp-types';
import type { WebMCPNotification } from '../mcp/web-worker/message';
import {
  ServiceContext,
  ServiceContextOptions,
  ServiceMetadata,
} from '../../features/tools';

/**
 * Static imports for all Web MCP server modules.
 *
 * Each server module exports a WebMCPServer instance as the default export.
 * Server metadata (displayName, description, category) is included directly
 * as properties on the server instance.
 */
import planningServer from './modules/planning-server/index.ts';
import playbookStore from './modules/playbook-store/index.ts';
import uiTools from './modules/ui-tools/index.ts';
import bootstrapServer from './modules/bootstrap-server/index.ts';
import mcpManagerServer from './modules/mcp-manager/index.ts';
import assistantManagerServer from './modules/assistant-manager/index.ts';
import knowledgeServer from './modules/knowledge-server/index.ts';

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

/**
 * Central registry of all Web MCP servers.
 *
 * This registry maps server keys (used as identifiers) to their WebMCPServer instances.
 * Each server instance contains metadata directly as properties (displayName, description, category).
 *
 * When adding a new server:
 * 1. Import the server above: `import newServer from './modules/new-server'`
 * 2. Add to this registry: `{ key: 'new_server', server: newServer }`
 * 3. The server's metadata will be automatically extracted from its properties
 */
const MODULE_REGISTRY = [
  { key: 'planning', server: planningServer },
  { key: 'playbook', server: playbookStore },
  { key: 'ui', server: uiTools },
  { key: 'bootstrap', server: bootstrapServer },
  { key: 'mcp_manager', server: mcpManagerServer },
  { key: 'assistant_manager', server: assistantManagerServer },
  { key: 'knowledge', server: knowledgeServer },
] as const;

/**
 * Map of server keys to their WebMCPServer instances.
 *
 * This is initialized directly from MODULE_REGISTRY for fast lookups
 * during tool execution and server context operations.
 */
const serverInstances = new Map<string, WebMCPServer>(
  MODULE_REGISTRY.map(({ key, server }) => [key, server]),
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
          const mcpResult = await server.callTool(toolName, args);

          // Log tool call completion (without full result for performance)
          log.debug('Tool call completed', { id, serverName, toolName });

          // Wrap MCPResult in JSON-RPC MCPResponse envelope
          const response = {
            jsonrpc: '2.0' as const,
            id,
            result: mcpResult,
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
        // Return empty context instead of generic placeholder
        // Only servers with meaningful state should implement getServiceContext
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [
              {
                type: 'text',
                text: '', // Empty - no context to provide
              },
            ],
            structuredContent: {
              contextPrompt: '',
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

      case 'getMetadata': {
        if (!serverName) {
          throw new Error('Server name is required for getMetadata');
        }

        const server = getMCPServer(serverName);

        /**
         * Build metadata from server's flat properties.
         */
        const metadata: ServiceMetadata = {
          displayName: server.displayName,
          description: server.description,
          icon: server.icon,
        };

        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: JSON.stringify(metadata) }],
            structuredContent: metadata,
          },
        };
      }

      case 'listServers': {
        /**
         * Build a list of all available servers with their metadata.
         *
         * For each server in MODULE_REGISTRY, we extract:
         * - name: Server key/identifier
         * - metadata: Built from server's flat properties
         * - toolCount: Number of tools the server provides
         */
        const serverList = MODULE_REGISTRY.map(({ key, server }) => {
          return {
            name: key,
            metadata: {
              displayName: server.displayName,
              description: server.description,
              icon: server.icon,
            },
            toolCount: server.tools?.length || 0,
          };
        });

        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: JSON.stringify(serverList) }],
            structuredContent: serverList,
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

/**
 * Send a notification from Worker to Main Thread
 * @param notifyType Type of notification
 * @param data Notification payload
 */
export function sendNotification(notifyType: string, data?: unknown): void {
  const notification: WebMCPNotification = {
    type: 'notify',
    notifyType,
    data,
  };
  self.postMessage(notification);
  log.debug(`Notification sent: ${notifyType}`, data);
}

// Export for use in server modules
(
  self as typeof self & { sendNotification: typeof sendNotification }
).sendNotification = sendNotification;

// Initialize worker
log.info('Initializing WebMCP worker');
log.info('WebMCP worker ready - servers loaded via static imports');

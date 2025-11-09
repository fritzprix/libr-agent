/**
 * @file Web Worker implementation for running MCP (Model Context Protocol) servers.
 *
 * This script runs in a separate thread as a Web Worker, providing an isolated
 * environment for executing MCP-compatible servers and tools without blocking the
 * main UI thread. It communicates with the main application using `postMessage`.
 *
 * ## Module Import Strategy
 *
 * We use **namespace imports** (`import * as moduleName`) instead of default-only imports
 * for the following reasons:
 *
 * 1. **Access to both exports**: Each module exports both a default export (the server instance)
 *    and named exports (metadata, tools). Namespace imports give us access to both.
 *
 * 2. **Runtime metadata extraction**: The `metadata` named export from each module
 *    (e.g., `planningModule.metadata`) provides UI display information (displayName,
 *    description, category) that is shown in the BuiltInToolsEditor.
 *
 * 3. **Type safety**: Static imports provide better type checking and bundler compatibility
 *    compared to dynamic imports.
 *
 * ## Module Structure Example
 *
 * Each MCP server module exports:
 * ```typescript
 * // modules/planning-server/index.ts
 * export { default } from './server.ts';        // Default: WebMCPServer instance
 * export { metadata } from './metadata';        // Named: ServiceMetadata object
 * export { planningTools } from './tools.ts';   // Named: Tool definitions
 * ```
 *
 * ## Adding New Servers
 *
 * To add a new Web MCP server:
 * 1. Create the module directory with index.ts, server.ts, metadata.ts, tools.ts
 * 2. Add namespace import: `import * as newModule from './modules/new-server/index.ts';`
 * 3. Register in MODULE_REGISTRY: `{ key: 'new_server', module: newModule }`
 * 4. The metadata will be automatically discovered and displayed in the UI
 */

import type {
  WebMCPServer,
  WebMCPMessage,
  MCPResponse,
  MCPTool,
} from '../mcp-types';
import {
  ServiceContext,
  ServiceContextOptions,
  ServiceMetadata,
} from '../../features/tools';

/**
 * Static namespace imports for all Web MCP server modules.
 *
 * IMPORTANT: Use namespace imports (`import * as`) to access both:
 * - module.default: The WebMCPServer instance
 * - module.metadata: The ServiceMetadata object for UI display
 * - module.tools: Tool definitions (if exported)
 *
 * DO NOT use default-only imports like `import planningServer from '...'`
 * as they won't give access to the metadata named export.
 */
import * as planningModule from './modules/planning-server/index.ts';
import * as playbookModule from './modules/playbook-store/index.ts';
import * as uiModule from './modules/ui-tools/index.ts';
import * as bootstrapModule from './modules/bootstrap-server/index.ts';
import * as mcpManagerModule from './modules/mcp-manager/index.ts';

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
 * Central registry of all Web MCP server modules.
 *
 * This registry maps server keys (used as identifiers) to their module namespaces.
 * Each module namespace contains:
 * - default: The WebMCPServer instance
 * - metadata: ServiceMetadata (displayName, description, category, icon)
 * - tools: Tool definitions (optional)
 *
 * The registry is used by:
 * - serverInstances: To create a map of server instances (module.default)
 * - getMetadata handler: To extract metadata for UI display (module.metadata)
 * - listServers handler: To provide complete server information to the frontend
 *
 * When adding a new server:
 * 1. Import the module namespace above: `import * as newModule from './modules/new-server'`
 * 2. Add to this registry: `{ key: 'new_server', module: newModule }`
 * 3. No other changes needed - metadata will be auto-discovered
 */
const MODULE_REGISTRY = [
  { key: 'planning', module: planningModule },
  { key: 'playbook', module: playbookModule },
  { key: 'ui', module: uiModule },
  { key: 'bootstrap', module: bootstrapModule },
  { key: 'mcp_manager', module: mcpManagerModule },
] as const;

/**
 * Map of server keys to their WebMCPServer instances.
 *
 * This is initialized by extracting the default export (server instance)
 * from each module namespace in MODULE_REGISTRY.
 *
 * Example: planningModule.default → WebMCPServer instance for 'planning'
 *
 * This map is used by getMCPServer() for fast server instance lookups
 * during tool execution and server context operations.
 */
const serverInstances = new Map<string, WebMCPServer>(
  MODULE_REGISTRY.map(({ key, module }) => [key, module.default]),
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

      case 'getMetadata': {
        if (!serverName) {
          throw new Error('Server name is required for getMetadata');
        }

        const server = getMCPServer(serverName);
        const registryEntry = MODULE_REGISTRY.find(
          (entry) => entry.key === serverName,
        );

        /**
         * Extract metadata from the module's named export.
         *
         * IMPORTANT: We cast to include both `default` and `metadata` because
         * TypeScript doesn't infer namespace import structure automatically.
         *
         * - registryEntry.module.default: WebMCPServer instance
         * - registryEntry.module.metadata: ServiceMetadata from metadata.ts file
         *
         * Fallback: If metadata export doesn't exist, use server properties.
         * This ensures backwards compatibility with older server implementations.
         */
        const metadata: ServiceMetadata = (
          registryEntry?.module as {
            default: WebMCPServer;
            metadata?: ServiceMetadata;
          }
        ).metadata || {
          displayName: server.name,
          description: server.description || '',
          category: 'automation',
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
         * For each module in MODULE_REGISTRY, we extract:
         * - name: Server key/identifier
         * - metadata: From module.metadata (named export) or fallback
         * - toolCount: Number of tools the server provides
         *
         * The metadata extraction uses the same pattern as getMetadata handler:
         * we cast the module to access both default and named exports.
         */
        const serverList = MODULE_REGISTRY.map(({ key, module }) => {
          const typedModule = module as {
            default: WebMCPServer;
            metadata?: ServiceMetadata;
            tools?: MCPTool[];
          };
          const server = getMCPServer(key);

          return {
            name: key,
            metadata: typedModule.metadata || {
              displayName: key,
              description: server.description || `MCP server: ${key}`,
              category: 'automation' as const,
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

// Initialize worker
log.info('Initializing WebMCP worker');
log.info('WebMCP worker ready - servers loaded via static imports');

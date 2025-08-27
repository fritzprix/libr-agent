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
  WebMCPResponse,
  MCPTool,
  SamplingOptions,
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

const MODULE_REGISTRY = [
  { key: 'content-store', importPath: './modules/content-store' },
  { key: 'planning-server', importPath: './modules/planning-server' },
  // Future modules can be added here
] as const;

const serverInstances = new Map<string, WebMCPServer | null>(
  MODULE_REGISTRY.map(({ key }) => [key, null]),
);

async function loadServers(): Promise<void> {
  try {
    log.debug('Loading MCP servers');

    const modulePromises = MODULE_REGISTRY.map(
      ({ importPath }) => import(importPath),
    );
    const results = await Promise.allSettled(modulePromises);

    results.forEach((result, index) => {
      const { key } = MODULE_REGISTRY[index];
      if (result.status === 'fulfilled') {
        serverInstances.set(key, result.value.default);
        log.debug(`${key} server loaded`);
      } else {
        log.error(`Failed to load ${key} server`, result.reason);
      }
    });

    log.info('Server loading completed');
  } catch (error) {
    log.error('Critical error during server loading', error);
  }
}

const getServerRegistry = (): Map<string, WebMCPServer | null> => {
  return serverInstances;
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
    if (Array.from(serverInstances.values()).every((s) => s === null)) {
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
async function handleMCPMessage(
  message: WebMCPMessage,
): Promise<WebMCPResponse> {
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
          for (const server of mcpServers.values()) {
            allTools.push(...server.tools);
          }
          return { id, result: allTools };
        } else {
          // Return tools from specific server
          const server = await loadMCPServer(serverName);
          return { id, result: server.tools };
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
            result,
          });

          // Log the detailed tool result for debugging/UI inspection
          log.info('callTool result', { id, serverName, toolName, result });

          // Tool 결과를 간단한 형태로 반환
          return { id, result };
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
            id,
            error:
              toolError instanceof Error
                ? toolError.message
                : String(toolError),
          };
        }
      }

      case 'sampleText': {
        if (!serverName) {
          throw new Error('Server name is required for sampleText');
        }
        const { prompt, options } = args as {
          prompt: string;
          options?: SamplingOptions;
        };
        const server = await loadMCPServer(serverName);

        // Web MCP 서버에 sampling 메서드가 있는지 확인
        if ('sampleText' in server && typeof server.sampleText === 'function') {
          const result = await server.sampleText(prompt, options);
          log.debug('Text sampling completed', {
            id,
            serverName,
            prompt: prompt.substring(0, 100) + '...',
          });
          return { id, result };
        } else {
          throw new Error(
            `Server ${serverName} does not support text sampling`,
          );
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

    return { id, error: errorMessage };
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

    const errorResponse: WebMCPResponse = {
      id: messageId,
      error: `Worker error: ${errorMessage}`,
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

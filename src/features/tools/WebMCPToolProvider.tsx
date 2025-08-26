import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useAsyncFn } from 'react-use';

// Vite Worker import
import { getLogger } from '@/lib/logger';
import { MCPResponse, WebMCPServerState } from '@/lib/mcp-types';
import { createWebMCPProxy, WebMCPProxy } from '@/lib/web-mcp/mcp-proxy';
import { ToolCall } from '@/models/chat';
import { BuiltInService, useBuiltInTool } from '.';
import MCPWorker from '../../lib/web-mcp/mcp-worker.ts?worker';

const logger = getLogger('WebMCPToolProvider');

interface WebMCPProviderProps {
  servers: string[];
}

export function WebMCPProvider({ servers = [] }: WebMCPProviderProps) {
  const serverStatesRef = useRef<Record<string, WebMCPServerState>>({});
  const proxyRef = useRef<WebMCPProxy | null>(null);
  const [initialized, setInitialized] = useState(false);
  const { register, unregister } = useBuiltInTool();

  // Initialize the Web Worker MCP proxy
  const [{ loading: isLoading }, initializeProxy] = useAsyncFn(async () => {
    try {
      logger.debug('Initializing WebMCP proxy with Vite worker');

      // Create Worker instance using Vite's ?worker import
      const workerInstance = new MCPWorker();

      // Create new proxy instance with Worker instance
      const proxy = createWebMCPProxy({ workerInstance });
      await proxy.initialize();

      proxyRef.current = proxy;
      setInitialized(true);
      logger.info('WebMCP proxy initialized successfully');
    } catch (error) {
      logger.error('Failed to initialize WebMCP proxy', error);
      setInitialized(false);
      throw error;
    }
  }, []);

  // Load a specific MCP server
  const loadServer = useCallback(async (serverName: string) => {
    if (!proxyRef.current) {
      throw new Error('WebMCP proxy not initialized');
    }

    try {
      logger.debug('Loading MCP server', { serverName });

      // Update server state to loading
      serverStatesRef.current = {
        ...serverStatesRef.current,
        [serverName]: {
          loaded: false,
          tools: [],
          lastActivity: Date.now(),
        },
      };

      // Load the server
      const serverInfo = await proxyRef.current.loadServer(serverName);

      // Get tools for this server and add external. prefix
      const tools = await proxyRef.current.listTools(serverName);
      serverStatesRef.current[serverName].tools = tools;

      logger.info('MCP server loaded successfully', {
        serverName,
        toolCount: tools.length,
        serverInfo,
      });
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Failed to load MCP server', { serverName, error });

      // Update server state with error
      serverStatesRef.current = {
        ...serverStatesRef.current,
        [serverName]: {
          loaded: false,
          tools: [],
          lastError: errorMessage,
          lastActivity: Date.now(),
        },
      };
      throw error;
    }
  }, []);

  // Call a tool on a specific server
  const executeTool = useCallback(
    async (serviceId: string, call: ToolCall): Promise<MCPResponse> => {
      if (!proxyRef.current) {
        throw new Error('WebMCP proxy not initialized');
      }
      // Handle external. prefix for namespace routing

      const result = await proxyRef.current.callTool(
        serviceId,
        call.function.name,
        JSON.parse(call.function.arguments),
      );

      // Log returned result from worker/proxy for debugging and to inspect payload shape
      logger.info('WebMCPToolProvider executeTool result', { serviceId, call, result });

      return {
        id: result.id,
        jsonrpc: '2.0',
        error: result.error
          ? {
              code:
                typeof result.error === 'object' && 'code' in result.error
                  ? (result.error as { code: number }).code
                  : -1,
              message:
                typeof result.error === 'string'
                  ? result.error
                  : typeof result.error === 'object' &&
                      'message' in result.error
                    ? (result.error as { message: string }).message
                    : String(result.error),
              data:
                typeof result.error === 'object' && 'data' in result.error
                  ? (result.error as { data?: unknown }).data
                  : undefined,
            }
          : undefined,
        result: {
          content: [{ type: 'text', text: JSON.stringify(result.result) }],
        },
      };
    },
    [],
  );

  const services: Record<string, BuiltInService> = useMemo(() => {
    if (isLoading) {
      return {};
    }
    return servers.reduce<Record<string, BuiltInService>>((acc, s) => {
      acc[s] = {
        executeTool: (tc) => executeTool(s, tc),
        loadService: () => loadServer(s),
        listTools: () => serverStatesRef.current[s]?.tools || [],
        unloadService: async () => {},
      };
      return acc;
    }, {});
  }, [servers, executeTool, loadServer, isLoading]);

  useEffect(() => {
    if (!isLoading && services && Object.entries(services).length > 0) {
      Object.entries(services).forEach(([id, service]) => {
        register(id, service);
      });
    }
    return () => {
      if (initialized && services && Object.entries(services).length > 0) {
        Object.entries(services).forEach(([id]) => {
          unregister(id);
        });
      }
    };
  }, [isLoading, initialized]);

  useEffect(() => {
    initializeProxy();
    return () => {
      proxyRef.current?.cleanup();
    };
  }, []);

  return null;
}

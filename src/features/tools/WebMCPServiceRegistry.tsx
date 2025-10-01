import { useCallback, useEffect, useMemo, useRef } from 'react';
import { getLogger } from '@/lib/logger';
import { MCPResponse, WebMCPServerState } from '@/lib/mcp-types';
import { ToolCall } from '@/models/chat';
import { BuiltInService, useBuiltInTool } from '.';
import { useWebMCP } from '@/context/WebMCPContext';

const logger = getLogger('WebMCPServiceRegistry');

interface WebMCPServiceRegistryProps {
  servers: string[];
}

export function WebMCPServiceRegistry({
  servers = [],
}: WebMCPServiceRegistryProps) {
  const serverStatesRef = useRef<Record<string, WebMCPServerState>>({});
  const { proxy, isLoading, initialized, getServerProxy } = useWebMCP();
  const { register, unregister } = useBuiltInTool();

  // Load a specific MCP server
  const loadServer = useCallback(
    async (serverName: string) => {
      if (!proxy) {
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

        // Load the server using context's getServerProxy
        const serverProxy = await getServerProxy(serverName);

        // Update server state with loaded tools
        serverStatesRef.current[serverName] = {
          loaded: true,
          tools: serverProxy.tools,
          lastActivity: Date.now(),
        };

        logger.info('MCP server loaded successfully', {
          serverName,
          toolCount: serverProxy.tools.length,
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
    },
    [proxy, getServerProxy],
  );

  // Call a tool on a specific server
  const executeTool = useCallback(
    async (
      serviceId: string,
      call: ToolCall,
    ): Promise<MCPResponse<unknown>> => {
      if (!proxy) {
        throw new Error('WebMCP proxy not initialized');
      }

      const result = await proxy.callTool(
        serviceId,
        call.function.name,
        JSON.parse(call.function.arguments),
      );

      // Log returned result from worker/proxy for debugging
      logger.info('WebMCPServiceRegistry executeTool result', {
        serviceId,
        call,
        result,
      });

      // proxy.callTool now returns MCPResponse directly
      return result;
    },
    [proxy],
  );

  // Create BuiltInService instances for each server
  const services: Record<string, BuiltInService> = useMemo(() => {
    if (isLoading || !initialized) {
      return {};
    }
    return servers.reduce<Record<string, BuiltInService>>((acc, s) => {
      // Determine metadata based on server name
      let metadata: {
        displayName: string;
        description: string;
        category: 'automation' | 'storage' | 'planning' | 'execution';
      } = {
        displayName: s,
        description: `Web MCP server: ${s}`,
        category: 'automation',
      };

      // Override metadata for known servers
      if (s === 'content_store') {
        metadata = {
          displayName: 'Content Store',
          description: 'File storage, search, BM25 indexing',
          category: 'storage',
        };
      } else if (s === 'workspace') {
        metadata = {
          displayName: 'Workspace',
          description: 'File read/write, code execution, search',
          category: 'storage',
        };
      } else if (s === 'planning') {
        metadata = {
          displayName: 'Task Planning',
          description: 'Goal setting, task planning',
          category: 'planning',
        };
      } else if (s === 'playbook') {
        metadata = {
          displayName: 'Playbook',
          description: 'Workflow creation and execution',
          category: 'execution',
        };
      }

      acc[s] = {
        metadata,
        executeTool: (tc) => executeTool(s, tc),
        listTools: () => serverStatesRef.current[s]?.tools || [],
        unloadService: async () => {},
        loadService: async () => loadServer(s),
        getServiceContext: () =>
          proxy ? proxy.getServiceContext(s) : Promise.resolve(''),
      };
      return acc;
    }, {});
  }, [servers, executeTool, loadServer, isLoading, initialized, proxy]);

  // Register services with BuiltInToolProvider
  useEffect(() => {
    if (
      !isLoading &&
      initialized &&
      services &&
      Object.entries(services).length > 0
    ) {
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
  }, [isLoading, initialized, services, register, unregister]);

  return null;
}

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getLogger } from '@/lib/logger';
import { MCPResponse, WebMCPServerState } from '@/lib/mcp-types';
import { ToolCall } from '@/models/chat';
import {
  BuiltInService,
  useBuiltInTool,
  ServiceContextOptions,
  ServiceContext,
  ServiceMetadata,
} from '.';
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
  const [serverMetadata, setServerMetadata] = useState<
    Record<string, ServiceMetadata>
  >({});

  // Load metadata from worker on initialization
  useEffect(() => {
    if (!initialized || !proxy) return;

    const currentProxy = proxy; // Capture proxy to avoid null check issues

    async function loadMetadata() {
      try {
        logger.debug('Loading server metadata from worker...');
        const serverList = await currentProxy.listAvailableServers();
        const metadataMap = serverList.reduce(
          (acc, server) => {
            acc[server.name] = server.metadata;
            return acc;
          },
          {} as Record<string, ServiceMetadata>,
        );
        setServerMetadata(metadataMap);
        logger.info('Server metadata loaded', {
          serverCount: serverList.length,
          servers: serverList.map((s) => s.name),
        });
      } catch (error) {
        logger.error('Failed to load server metadata', error);
      }
    }

    loadMetadata();
  }, [initialized, proxy]);

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
    return servers.reduce<Record<string, BuiltInService>>((acc, serverName) => {
      // Get metadata from state or use default
      const metadata: ServiceMetadata = serverMetadata[serverName] || {
        displayName: serverName,
        description: `Web MCP server: ${serverName}`,
        category: 'automation',
      };

      acc[serverName] = {
        metadata,
        executeTool: (tc) => executeTool(serverName, tc),
        listTools: () => serverStatesRef.current[serverName]?.tools || [],
        unloadService: async () => {},
        loadService: async () => loadServer(serverName),
        getServiceContext: async (
          options?: ServiceContextOptions,
        ): Promise<ServiceContext<unknown>> => {
          if (!proxy) {
            return { contextPrompt: '', structuredState: undefined };
          }
          return await proxy.getServiceContext(serverName, options);
        },
        switchContext: async (options?: ServiceContextOptions) => {
          if (proxy && proxy.switchContext) {
            await proxy.switchContext(
              serverName,
              (options || {}) as ServiceContextOptions,
            );
          }
        },
      };
      return acc;
    }, {});
  }, [
    servers,
    executeTool,
    loadServer,
    isLoading,
    initialized,
    proxy,
    serverMetadata,
  ]);

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

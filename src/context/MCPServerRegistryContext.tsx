import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { mutate } from 'swr';
import { MCPServerEntity } from '@/models/chat';
import { McpServerService } from '@/lib/services/mcp-server-service';
import { useSettings } from '@/hooks/use-settings';
import { getLogger } from '@/lib/logger';

const logger = getLogger('MCPServerRegistryContext');

/**
 * Context type for MCP Server Registry
 * Provides SWR-centric state management for MCP server configurations
 */
export interface MCPServerRegistryContextType {
  // All servers cached in memory
  allServers: MCPServerEntity[];
  // Filtered active servers
  activeServers: MCPServerEntity[];
  // Loading state
  loading: boolean;
  // Error state
  error?: string;

  // Actions
  saveServer: (server: MCPServerEntity) => Promise<void>;
  deleteServer: (id: string) => Promise<void>;
  toggleActive: (id: string, active: boolean) => Promise<void>;
  refreshAll: () => Promise<void>;
}

const MCPServerRegistryContext = createContext<
  MCPServerRegistryContextType | undefined
>(undefined);

/**
 * Invalidates all SWR cache keys related to MCP servers
 * This triggers re-fetching in useSWRInfinite hooks
 */
const invalidateMCPServerPages = async () => {
  await mutate((key) => Array.isArray(key) && key[0] === 'mcpServers');
};

// Window event broadcasting removed in favor of React state propagation via context

export const MCPServerRegistryProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const [allServers, setAllServers] = useState<MCPServerEntity[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const { value: settings } = useSettings();

  // Use service layer
  const mcpServerService = useMemo(() => {
    return new McpServerService(settings.agentHubUrl);
  }, [settings.agentHubUrl]);

  // Use ref to avoid stale closures in event handlers
  const allServersRef = useRef<MCPServerEntity[]>([]);

  useEffect(() => {
    allServersRef.current = allServers;
  }, [allServers]);

  /**
   * Loads all MCP servers from the database
   * This provides the full list for filtering and reference
   */
  const refreshAll = useCallback(async () => {
    setLoading(true);
    try {
      const servers = await mcpServerService.getAll();
      setAllServers(servers);
      setError(undefined);
      logger.debug(`Loaded ${servers.length} MCP servers from service`);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      logger.error('Failed to load all MCP servers', err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [mcpServerService]);

  /**
   * Saves or updates an MCP server
   * Triggers cache invalidation and change broadcast
   */
  const saveServer = useCallback(
    async (server: MCPServerEntity) => {
      try {
        await mcpServerService.save(server);
        // No need to manually call refreshAll - event listener will handle it
        await invalidateMCPServerPages();
        logger.info(`MCP server "${server.name}" saved successfully`);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        logger.error('Failed to save MCP server', err);
        throw new Error(`Failed to save server: ${message}`);
      }
    },
    [mcpServerService],
  );

  /**
   * Deletes an MCP server
   * Enforces referential integrity - prevents deletion if referenced by assistants
   */
  const deleteServer = useCallback(
    async (id: string) => {
      try {
        await mcpServerService.delete(id);
        // No need to manually call refreshAll - event listener will handle it
        await invalidateMCPServerPages();
        logger.info(`MCP server with ID "${id}" deleted successfully`);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        logger.error('Failed to delete MCP server', err);
        throw new Error(`Failed to delete server: ${message}`);
      }
    },
    [mcpServerService],
  );

  /**
   * Toggles the active status of an MCP server
   */
  const toggleActive = useCallback(
    async (id: string, active: boolean) => {
      const server = allServersRef.current.find((s) => s.id === id);
      if (!server) {
        throw new Error(`MCP server with ID "${id}" not found`);
      }

      await saveServer({ ...server, isActive: active });
    },
    [saveServer],
  );

  /**
   * Filtered list of active servers
   * Memoized to avoid unnecessary re-renders
   */
  const activeServers = useMemo(
    () => allServers.filter((s) => s.isActive),
    [allServers],
  );

  // Subscribe to local service events (Main Thread changes)
  useEffect(() => {
    const unsubscribe = mcpServerService.onRevalidate((event) => {
      logger.debug('Local service changed, refreshing...', event);
      refreshAll();
    });
    return unsubscribe;
  }, [mcpServerService, refreshAll]);

  // Initial load on mount
  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const value: MCPServerRegistryContextType = useMemo(
    () => ({
      allServers,
      activeServers,
      loading,
      error,
      saveServer,
      deleteServer,
      toggleActive,
      refreshAll,
    }),
    [
      allServers,
      activeServers,
      loading,
      error,
      saveServer,
      deleteServer,
      toggleActive,
      refreshAll,
    ],
  );

  return (
    <MCPServerRegistryContext.Provider value={value}>
      {children}
    </MCPServerRegistryContext.Provider>
  );
};

/**
 * Hook to access MCP Server Registry context
 * @throws Error if used outside of MCPServerRegistryProvider
 */
export function useMCPServerRegistry() {
  const context = useContext(MCPServerRegistryContext);
  if (!context) {
    throw new Error(
      'useMCPServerRegistry must be used within MCPServerRegistryProvider',
    );
  }
  return context;
}

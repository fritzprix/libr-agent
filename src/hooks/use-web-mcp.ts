import { useWebMCP as useWebMCPContext, useWebMCPManagement as useWebMCPManagementContext } from '../context/WebMCPContext';

/**
 * 🌐 Hook for using Web MCP functionality
 *
 * Provides access to Web Worker-based MCP servers and tools.
 * Must be used within a WebMCPProvider.
 */
export const useWebMCP = () => {
  return useWebMCPContext();
};

/**
 * 🛠️ Hook for simplified Web MCP tool operations
 *
 * Provides convenience methods for common MCP operations.
 */
export const useWebMCPTools = () => {
  const { availableTools, callTool, serverStates, isInitialized, error } =
    useWebMCP();

  /**
   * Get all available tools with server prefix
   */
  const getAvailableTools = () => availableTools;

  /**
   * Find a specific tool by name (with server prefix)
   */
  const findTool = (toolName: string) => {
    return availableTools.find((tool) => tool.name === toolName);
  };

  /**
   * Get tools from a specific server
   */
  const getServerTools = (serverName: string) => {
    const serverState = serverStates[serverName];
    return serverState?.tools || [];
  };

  /**
   * Execute a tool call with automatic server name extraction
   * Supports both "serverName__toolName" and separate parameters
   */
  const executeCall = async (
    serverNameOrFullName: string,
    toolNameOrArgs?: string | unknown,
    args?: unknown,
  ): Promise<unknown> => {
    let serverName: string;
    let toolName: string;
    let toolArgs: unknown;

    // Parse arguments based on format
    if (typeof toolNameOrArgs === 'string') {
      // Format: executeCall("serverName", "toolName", args)
      serverName = serverNameOrFullName;
      toolName = toolNameOrArgs;
      toolArgs = args;
    } else {
      // Format: executeCall("serverName__toolName", args)
      const parts = serverNameOrFullName.split('__');
      if (parts.length < 2) {
        throw new Error(
          `Invalid tool name format. Expected "serverName__toolName", got "${serverNameOrFullName}"`,
        );
      }
      serverName = parts[0];
      toolName = parts.slice(1).join('__');
      toolArgs = toolNameOrArgs;
    }

    if (!serverName || !toolName) {
      throw new Error('Server name and tool name are required');
    }

    return await callTool(serverName, toolName, toolArgs);
  };

  /**
   * Check if a server is loaded and available
   */
  const isServerLoaded = (serverName: string): boolean => {
    return serverStates[serverName]?.loaded || false;
  };

  /**
   * Get server status information
   */
  const getServerStatus = (serverName: string) => {
    return serverStates[serverName] || null;
  };

  /**
   * Check if the Web MCP system is ready for use
   */
  const isReady = isInitialized && !error;

  return {
    // Tool access
    availableTools,
    getAvailableTools,
    findTool,
    getServerTools,

    // Tool execution
    executeCall,

    // Server status
    serverStates,
    isServerLoaded,
    getServerStatus,

    // System status
    isReady,
    isInitialized,
    error,
  };
};

/**
 * 🔄 Hook for managing Web MCP servers
 *
 * Provides methods for loading and managing MCP servers.
 */
export const useWebMCPManagement = () => {
  const {
    loadServer,
    cleanup,
    initializeProxy,
  } = useWebMCPManagementContext();
  
  const {
    getProxyStatus,
    isInitialized,
    isLoading,
    error,
  } = useWebMCPContext();

  /**
   * Load multiple servers concurrently
   */
  const loadServers = async (serverNames: string[]): Promise<void> => {
    await Promise.all(serverNames.map((serverName) => loadServer(serverName)));
  };

  /**
   * Reinitialize the Web MCP system
   */
  const reinitialize = async (): Promise<void> => {
    cleanup();
    await initializeProxy();
  };

  /**
   * Get detailed system status
   */
  const getSystemStatus = () => {
    const proxyStatus = getProxyStatus();
    return {
      initialized: isInitialized,
      loading: isLoading,
      error,
      proxy: proxyStatus,
    };
  };

  return {
    // Server management
    loadServer,
    loadServers,

    // System management
    cleanup,
    reinitialize,
    initializeProxy,

    // Status
    getSystemStatus,
    getProxyStatus,
    isInitialized,
    isLoading,
    error,
  };
};

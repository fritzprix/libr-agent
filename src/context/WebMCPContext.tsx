import React, {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  useRef,
} from 'react';
import { useAsyncFn } from 'react-use';
import { getLogger } from '../lib/logger';
import { WebMCPProxy, createWebMCPProxy } from '../lib/web-mcp/mcp-proxy';
import { MCPTool, WebMCPServerState } from '../lib/mcp-types';

// Vite Worker import
import MCPWorker from '../lib/web-mcp/mcp-worker.ts?worker';

const logger = getLogger('WebMCPContext');

// 공개 API (구독자용)
interface WebMCPContextType {
  availableTools: MCPTool[];
  serverStates: Record<string, WebMCPServerState>;
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;

  // 핵심 기능만 노출
  callTool: (
    serverName: string,
    toolName: string,
    args: unknown,
  ) => Promise<unknown>;
  getProxyStatus: () => {
    initialized: boolean;
    pendingRequests: number;
    workerType: string;
  } | null;
}

// 내부 API (Provider 내부 + 초기화 컴포넌트용)
interface WebMCPInternalContextType extends WebMCPContextType {
  initializeProxy: () => Promise<void>;
  loadServer: (serverName: string) => Promise<void>;
  listTools: (serverName?: string) => Promise<MCPTool[]>;
  cleanup: () => void;
}

export const WebMCPContext = createContext<WebMCPContextType | undefined>(
  undefined,
);

const WebMCPInternalContext = createContext<WebMCPInternalContextType | undefined>(
  undefined,
);

// 초기화 전용 컴포넌트
function WebMCPInitializer() {
  const { isInitialized, isLoading, error, initializeProxy } = useContext(WebMCPInternalContext)!;

  useEffect(() => {
    if (!isInitialized && !isLoading && !error) {
      initializeProxy();
    }
  }, [isInitialized, isLoading, error, initializeProxy]);

  return null;
}

interface WebMCPProviderProps {
  children: ReactNode;
  servers?: string[];
  autoLoad?: boolean;
}

export const WebMCPProvider: React.FC<WebMCPProviderProps> = ({
  children,
  servers = [],
  autoLoad = true,
}) => {
  const [availableTools, setAvailableTools] = useState<MCPTool[]>([]);
  const [serverStates, setServerStates] = useState<
    Record<string, WebMCPServerState>
  >({});
  const [error, setError] = useState<string | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);

  const proxyRef = useRef<WebMCPProxy | null>(null);
  const availableToolsRef = useRef(availableTools);

  // Initialize the Web Worker MCP proxy
  const [{ loading: isLoading }, initializeProxy] = useAsyncFn(async () => {
    try {
      logger.debug('Initializing WebMCP proxy with Vite worker');
      setError(null);

      // Create Worker instance using Vite's ?worker import
      const workerInstance = new MCPWorker();

      // Create new proxy instance with Worker instance
      const proxy = createWebMCPProxy({ workerInstance });
      await proxy.initialize();

      proxyRef.current = proxy;
      setIsInitialized(true);

      logger.info('WebMCP proxy initialized successfully');

      // Auto-load servers if specified
      if (autoLoad && servers.length > 0) {
        await Promise.all(
          servers.map((serverName) => loadServerInternal(serverName)),
        );
      }
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Failed to initialize WebMCP proxy', error);
      setError(errorMessage);
      setIsInitialized(false);
      throw error;
    }
  }, [servers, autoLoad]);

  // Load a specific MCP server
  const loadServerInternal = useCallback(async (serverName: string) => {
    if (!proxyRef.current) {
      throw new Error('WebMCP proxy not initialized');
    }

    try {
      logger.debug('Loading MCP server', { serverName });

      // Update server state to loading
      setServerStates((prev) => ({
        ...prev,
        [serverName]: {
          loaded: false,
          tools: [],
          lastActivity: Date.now(),
        },
      }));

      // Load the server
      const serverInfo = await proxyRef.current.loadServer(serverName);

      // Get tools for this server
      const tools = await proxyRef.current.listTools(serverName);

      // Update server state
      setServerStates((prev) => ({
        ...prev,
        [serverName]: {
          loaded: true,
          tools,
          lastActivity: Date.now(),
        },
      }));

      // Update available tools
      const allTools = await proxyRef.current.listAllTools();
      setAvailableTools(allTools);

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
      setServerStates((prev) => ({
        ...prev,
        [serverName]: {
          loaded: false,
          tools: [],
          lastError: errorMessage,
          lastActivity: Date.now(),
        },
      }));

      throw error;
    }
  }, []);

  const loadServer = useCallback(
    async (serverName: string) => {
      await loadServerInternal(serverName);
    },
    [loadServerInternal],
  );

  // List tools from a server or all servers
  const listTools = useCallback(
    async (serverName?: string): Promise<MCPTool[]> => {
      if (!proxyRef.current) {
        throw new Error('WebMCP proxy not initialized');
      }

      try {
        if (serverName) {
          return await proxyRef.current.listTools(serverName);
        } else {
          return await proxyRef.current.listAllTools();
        }
      } catch (error) {
        logger.error('Failed to list tools', { serverName, error });
        throw error;
      }
    },
    [],
  );

  // Call a tool on a specific server
  const callTool = useCallback(
    async (
      serverName: string,
      toolName: string,
      args: unknown,
    ): Promise<unknown> => {
      if (!proxyRef.current) {
        throw new Error('WebMCP proxy not initialized');
      }

      try {
        logger.debug('Calling WebMCP tool', { serverName, toolName, args });

        const result = await proxyRef.current.callTool(
          serverName,
          toolName,
          args,
        );

        // Update server activity
        setServerStates((prev) => ({
          ...prev,
          [serverName]: {
            ...prev[serverName],
            lastActivity: Date.now(),
          },
        }));

        logger.debug('WebMCP tool call completed', {
          serverName,
          toolName,
          result,
        });
        return result;
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        logger.error('Failed to call WebMCP tool', {
          serverName,
          toolName,
          error,
        });

        // Update server state with error
        setServerStates((prev) => ({
          ...prev,
          [serverName]: {
            ...prev[serverName],
            lastError: errorMessage,
            lastActivity: Date.now(),
          },
        }));

        throw error;
      }
    },
    [],
  );

  // Cleanup resources
  const cleanup = useCallback(() => {
    logger.debug('Cleaning up WebMCP context');

    if (proxyRef.current) {
      proxyRef.current.cleanup();
      proxyRef.current = null;
    }

    setIsInitialized(false);
    setAvailableTools([]);
    setServerStates({});
    setError(null);
  }, []);

  // Get proxy status
  const getProxyStatus = useCallback(() => {
    return proxyRef.current?.getStatus() || null;
  }, []);

  // Update ref when availableTools changes
  useEffect(() => {
    availableToolsRef.current = availableTools;
  }, [availableTools]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      cleanup();
    };
  }, [cleanup]);

  const value: WebMCPContextType = useMemo(
    () => ({
      availableTools,
      serverStates,
      isInitialized,
      isLoading,
      error,
      callTool,
      getProxyStatus,
    }),
    [
      availableTools,
      serverStates,
      isInitialized,
      isLoading,
      error,
      callTool,
      getProxyStatus,
    ],
  );

  const internalValue: WebMCPInternalContextType = useMemo(
    () => ({
      availableTools,
      serverStates,
      isInitialized,
      isLoading,
      error,
      callTool,
      getProxyStatus,
      initializeProxy,
      loadServer,
      listTools,
      cleanup,
    }),
    [
      availableTools,
      serverStates,
      isInitialized,
      isLoading,
      error,
      callTool,
      getProxyStatus,
      initializeProxy,
      loadServer,
      listTools,
      cleanup,
    ],
  );

  return (
    <WebMCPInternalContext.Provider value={internalValue}>
      <WebMCPContext.Provider value={value}>
        {children}
        <WebMCPInitializer />
      </WebMCPContext.Provider>
    </WebMCPInternalContext.Provider>
  );
};

// 공개 hook
export const useWebMCP = () => {
  const context = useContext(WebMCPContext);
  if (context === undefined) {
    throw new Error('useWebMCP must be used within a WebMCPProvider');
  }
  return context;
};

// 내부 관리용 hook (같은 파일 내에서만 사용하거나, 특별한 경우에만 export)
export const useWebMCPManagement = () => {
  const context = useContext(WebMCPInternalContext);
  if (context === undefined) {
    throw new Error('useWebMCPManagement must be used within a WebMCPProvider');
  }
  return {
    initializeProxy: context.initializeProxy,
    loadServer: context.loadServer,
    listTools: context.listTools,
    cleanup: context.cleanup,
  };
};

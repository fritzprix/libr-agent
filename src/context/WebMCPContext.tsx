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

// 간단한 서버 프록시 인터페이스
export interface WebMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  [methodName: string]: unknown; // 동적 메서드들
}

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
  // 새로운 서버 프록시 API
  getWebMCPServer: (serverName: string) => Promise<WebMCPServerProxy>;
}

export const WebMCPContext = createContext<WebMCPContextType | undefined>(
  undefined,
);

const WebMCPInternalContext = createContext<
  WebMCPInternalContextType | undefined
>(undefined);

// 초기화 전용 컴포넌트
function WebMCPInitializer() {
  const { isInitialized, isLoading, error, initializeProxy } = useContext(
    WebMCPInternalContext,
  )!;

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
  const serverProxiesRef = useRef<Map<string, WebMCPServerProxy>>(new Map());

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

  // 서버 프록시 생성 함수
  const createServerProxy = useCallback(
    (serverName: string, tools: MCPTool[]): WebMCPServerProxy => {
      const serverProxy: WebMCPServerProxy = {
        name: serverName,
        isLoaded: true,
        tools,
      };

      // 각 툴에 대해 동적으로 메서드 생성
      logger.info('useWebMCPContext : ', { tools });
      tools.forEach((tool) => {
        // prefix가 붙어있다면 제거해서 메서드 이름으로 사용
        const originalToolName = tool.name;
        const methodName = originalToolName.startsWith(`${serverName}__`)
          ? originalToolName.replace(`${serverName}__`, '')
          : originalToolName;

        logger.debug('Processing tool for server proxy', {
          serverName,
          originalToolName,
          methodName,
        });

        serverProxy[methodName] = async (args?: unknown) => {
          if (!proxyRef.current) {
            throw new Error('WebMCP proxy not initialized');
          }

          logger.debug('Calling server method', {
            serverName,
            methodName,
            originalToolName,
            args,
          });

          try {
            // callTool에는 prefix 제거된 이름을 사용
            const result = await proxyRef.current.callTool(
              serverName,
              methodName,
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

            logger.debug('Server method call successful', {
              serverName,
              methodName,
              result,
            });

            return result;
          } catch (error) {
            const errorMessage =
              error instanceof Error ? error.message : String(error);
            logger.error('Server method call failed', {
              serverName,
              methodName,
              originalToolName,
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
        };

        logger.debug('Method assigned to server proxy', {
          serverName,
          originalToolName,
          methodName,
          methodType: typeof serverProxy[methodName],
          isFunction: typeof serverProxy[methodName] === 'function',
        });
      });

      return serverProxy;
    },
    [],
  );

  // MCP 서버 프록시 가져오기
  const getWebMCPServer = useCallback(
    async (serverName: string): Promise<WebMCPServerProxy> => {
      // 이미 캐시된 프록시가 있는지 확인
      const cachedProxy = serverProxiesRef.current.get(serverName);
      if (cachedProxy) {
        return cachedProxy;
      }

      // 서버가 로드되지 않았다면 로드
      if (!serverStates[serverName]?.loaded) {
        await loadServerInternal(serverName);
      }

      // 서버 상태 확인
      const serverState = serverStates[serverName];
      if (!serverState?.loaded) {
        throw new Error(`Server ${serverName} is not loaded`);
      }

      // 서버 프록시 생성
      const serverProxy = createServerProxy(serverName, serverState.tools);
      serverProxiesRef.current.set(serverName, serverProxy);

      logger.info('Created server proxy', {
        serverName,
        methodCount: serverState.tools.length,
      });

      return serverProxy;
    },
    [serverStates, createServerProxy, loadServerInternal],
  );

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

    // 서버 프록시 캐시 정리
    serverProxiesRef.current.clear();

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
      getWebMCPServer,
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
      getWebMCPServer,
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

// 내부 관리용 hook
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
    getWebMCPServer: context.getWebMCPServer,
    serverStates: context.serverStates,
  };
};

// 타입 안전한 Web MCP 서버 사용을 위한 새로운 hook
export function useWebMCPServer<T extends WebMCPServerProxy>(
  serverName: string,
) {
  const { getWebMCPServer, serverStates } = useWebMCPManagement();
  const [server, setServer] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadServerProxy = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const serverProxy = (await getWebMCPServer(serverName)) as T;
      setServer(serverProxy);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      logger.error('Failed to load server proxy', { serverName, error: err });
    } finally {
      setLoading(false);
    }
  }, [getWebMCPServer, serverName]);

  // 서버 상태가 변경되면 자동으로 프록시 로드
  useEffect(() => {
    const serverState = serverStates[serverName];
    if (serverState?.loaded && !server) {
      loadServerProxy();
    }
  }, [serverStates, serverName, server, loadServerProxy]);

  return {
    server,
    loading,
    error,
    serverState: serverStates[serverName],
    reload: loadServerProxy,
  } as const;
}

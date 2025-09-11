import React, {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { getLogger } from '@/lib/logger';
import { WebMCPProxy } from '@/lib/web-mcp/mcp-proxy';
import { MCPTool } from '@/lib/mcp-types';
import MCPWorker from '@/lib/web-mcp/mcp-worker.ts?worker';

const logger = getLogger('WebMCPContext');

// Server proxy interface
export interface WebMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  [methodName: string]: unknown;
}

// Context interface
interface WebMCPContextValue {
  proxy: WebMCPProxy | null;
  isLoading: boolean;
  initialized: boolean;
  getServerProxy: <T extends WebMCPServerProxy>(
    serverName: string,
  ) => Promise<T>;
}

const WebMCPContext = createContext<WebMCPContextValue | null>(null);

interface WebMCPProviderProps {
  children: React.ReactNode;
}

export function WebMCPProvider({ children }: WebMCPProviderProps) {
  const proxyRef = useRef<WebMCPProxy | null>(null);
  const serverProxiesRef = useRef<Map<string, WebMCPServerProxy>>(new Map());
  const [initialized, setInitialized] = useState(false);

  // Initialize the Web Worker MCP proxy
  const [{ loading: isLoading }, initializeProxy] = useAsyncFn(async () => {
    try {
      logger.debug('Initializing WebMCP proxy with Vite worker');

      // Create Worker instance using Vite's ?worker import
      const workerInstance = new MCPWorker();

      // Create new proxy instance with Worker instance
      const proxy = new WebMCPProxy({ workerInstance });
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

  // Get server proxy with caching
  const getServerProxy = useCallback(
    async <T extends WebMCPServerProxy>(serverName: string): Promise<T> => {
      // Return cached proxy if available
      const cachedProxy = serverProxiesRef.current.get(serverName);
      if (cachedProxy) {
        return cachedProxy as T;
      }

      if (!proxyRef.current) {
        throw new Error('WebMCP proxy not initialized');
      }

      const proxy = proxyRef.current;

      try {
        logger.debug('Loading server and creating proxy', { serverName });

        // Load server
        await proxy.loadServer(serverName);
        const tools = await proxy.listTools(serverName);

        // Create server proxy
        const serverProxy: WebMCPServerProxy = {
          name: serverName,
          isLoaded: true,
          tools,
        };

        // Add dynamic tool methods
        tools.forEach((tool) => {
          const methodName = tool.name.startsWith(`${serverName}__`)
            ? tool.name.replace(`${serverName}__`, '')
            : tool.name;

          serverProxy[methodName] = async (args?: unknown) => {
            let safeArgs: Record<string, unknown> | undefined;
            if (typeof args === 'undefined') {
              safeArgs = undefined;
            } else if (
              typeof args === 'object' &&
              args !== null &&
              !Array.isArray(args)
            ) {
              safeArgs = args as Record<string, unknown>;
            } else if (typeof args === 'string') {
              try {
                safeArgs = args.length
                  ? (JSON.parse(args) as Record<string, unknown>)
                  : {};
              } catch {
                safeArgs = { raw: args };
              }
            } else {
              safeArgs = { value: args };
            }

            const mcpResponse = await proxy.callTool(
              serverName,
              methodName,
              safeArgs,
            );

            // Handle MCP response processing (same logic as original)
            if (
              mcpResponse &&
              typeof mcpResponse === 'object' &&
              'error' in mcpResponse
            ) {
              throw new Error(
                (mcpResponse as { error: { message: string } }).error.message,
              );
            }

            if (
              mcpResponse &&
              typeof mcpResponse === 'object' &&
              'result' in mcpResponse
            ) {
              const result = (
                mcpResponse as {
                  result?: { content?: Array<{ type: string; text?: string }> };
                }
              ).result;
              if (
                result?.content?.[0]?.type === 'text' &&
                result.content[0].text
              ) {
                try {
                  return JSON.parse(result.content[0].text);
                } catch {
                  return result.content[0].text;
                }
              }
              return result;
            }

            return mcpResponse;
          };
        });

        // Cache the proxy
        serverProxiesRef.current.set(serverName, serverProxy);

        logger.info('Created server proxy', {
          serverName,
          toolCount: tools.length,
        });

        return serverProxy as T;
      } catch (error) {
        logger.error('Failed to create server proxy', { serverName, error });
        throw error;
      }
    },
    [],
  );

  // Initialize proxy on mount
  useEffect(() => {
    initializeProxy();
  }, [initializeProxy]);

  const contextValue: WebMCPContextValue = {
    proxy: proxyRef.current,
    isLoading,
    initialized,
    getServerProxy,
  };

  return (
    <WebMCPContext.Provider value={contextValue}>
      {children}
    </WebMCPContext.Provider>
  );
}

// Hook to use WebMCP context
export function useWebMCP(): WebMCPContextValue {
  const context = useContext(WebMCPContext);
  if (!context) {
    throw new Error('useWebMCP must be used within a WebMCPProvider');
  }
  return context;
}

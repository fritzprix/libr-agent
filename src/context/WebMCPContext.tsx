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
import { ServiceContextOptions } from '@/features/tools';
import MCPWorker from '@/lib/web-mcp/mcp-worker.ts?worker';

const logger = getLogger('WebMCPContext');

/**
 * Server proxy interface exposed to the UI layer.
 *
 * Dynamic tool methods are attached at runtime based on the server's tool list.
 * Tool names are normalized by stripping the `${serverName}__` prefix if present.
 *
 * Response contract (client-side):
 * - When a tool is called through this proxy, the return value is derived from the MCPResponse as follows:
 *   1) If `result.structuredContent` exists, it is returned as-is (preferred for typed data)
 *   2) Else if `result.content[0].type === 'text'`, attempts JSON.parse on `text`, falling back to the raw string
 *   3) Else returns the raw `result` object
 *
 * This ensures compatibility with both servers that return structured data
 * (e.g., content-store via `createMCPStructuredResponse`) and servers that
 * return text (e.g., planning-server via `createMCPTextResponse`).
 */
export interface WebMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  switchContext?: (
    context: ServiceContextOptions,
  ) => Promise<{ success: boolean }>;
  [methodName: string]: unknown;
}

/**
 * WebMCP context value shared via React Context.
 *
 * - `proxy`: Low-level worker proxy (message transport, lifecycle)
 * - `isLoading`: Initialization state of the worker proxy
 * - `initialized`: True once the worker proxy has successfully initialized
 * - `getServerProxy(serverName)`: Lazily loads a server in the worker and returns a cached
 *   dynamic proxy with typed tool methods.
 */
interface WebMCPContextValue {
  proxy: WebMCPProxy | null;
  isLoading: boolean;
  initialized: boolean;
  getServerProxy: <T extends WebMCPServerProxy>(
    serverName: string,
  ) => Promise<T>;
  switchServerContext: (
    serverName: string,
    context: ServiceContextOptions,
  ) => Promise<{ success: boolean }>;
}

const WebMCPContext = createContext<WebMCPContextValue | null>(null);

/**
 * Props for WebMCPProvider.
 *
 * Wrap your app with this provider to enable Web Worker-based MCP servers.
 * The provider initializes a single worker-backed `WebMCPProxy` and exposes
 * a cached `getServerProxy` that attaches dynamic tool methods per server.
 */
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
  /**
   * Loads an MCP server into the worker (if not already loaded) and returns a dynamic proxy.
   *
   * Tool method mapping:
   * - For each tool listed by the server, a method is attached to the proxy using the tool's
   *   name with `${serverName}__` prefix removed when present. Example:
   *   - Tool name `content-store__addContent` becomes `serverProxy.addContent(...)`
   *   - Tool name `planning__create_goal` becomes `serverProxy.create_goal(...)`
   *
   * Response handling precedence:
   * 1) Return `result.structuredContent` if available
   * 2) Else attempt to parse `result.content[0].text` as JSON; fallback to plain string
   * 3) Else return the raw `result` object
   */
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

            // Handle MCP response processing per the response contract
            if (mcpResponse.result && mcpResponse.result.structuredContent) {
              const { structuredContent } = mcpResponse.result;
              return structuredContent;
            }

            // Check if there's an error in the response
            if (mcpResponse.error) {
              throw new Error(
                `MCP tool execution failed: ${methodName} - ${mcpResponse.error.message} (code: ${mcpResponse.error.code})`,
              );
            }

            // If we get here, the response doesn't have structuredContent
            throw new Error(
              `MCP tool execution failed: ${methodName} - Server did not return structured content in the expected format`,
            );
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

  // Set context for a server
  const switchServerContext = useCallback(
    async (
      serverName: string,
      context: ServiceContextOptions,
    ): Promise<{ success: boolean }> => {
      if (!proxyRef.current) {
        throw new Error('WebMCP proxy not initialized');
      }

      try {
        const result = await proxyRef.current.switchContext(
          serverName,
          context,
        );
        logger.debug('Switched server context', { serverName, context });
        return result;
      } catch (error) {
        logger.error('Failed to switch server context', { serverName, error });
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
    switchServerContext,
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

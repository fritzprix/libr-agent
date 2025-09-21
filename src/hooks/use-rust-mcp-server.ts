import { useCallback, useEffect, useMemo, useState } from 'react';
import { getLogger } from '@/lib/logger';
import type { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { listBuiltinTools, callBuiltinTool } from '@/lib/rust-backend-client';

const logger = getLogger('useRustMCPServer');

/**
 * Server proxy interface exposed to the UI layer for Rust built-in MCP servers.
 * Mirrors `WebMCPServerProxy` so calling sites can swap hooks with minimal changes.
 */
export interface RustMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  // Dynamic tool methods are attached at runtime
  [methodName: string]: unknown;
}

/**
 * Hook: useRustMCPServer
 *
 * Purpose:
 * - Lazily obtain a dynamic proxy for a Rust built-in MCP server (Tauri backend).
 * - The returned proxy exposes methods for each tool the server provides.
 *
 * Parameters:
 * - serverName: string — The built-in MCP server name (e.g., "content-store" or "contentstore").
 *
 * Returns:
 * - { server, loading, error, reload }
 *   - server: T | null — Dynamic proxy with tool methods; null while loading/failed
 *   - loading: boolean — True while proxy/tools are loading
 *   - error: string | null — Last error message, if any
 *   - reload: () => Promise<void> — Force reload of the server proxy
 *
 * Response Contract:
 * - Returns `result.structuredContent` when present.
 * - If `error` is present in MCPResponse, throws an Error.
 */
export function useRustMCPServer<T extends RustMCPServerProxy>(
  serverName: string,
) {
  const [server, setServer] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Normalize common aliases to keep compatibility with docs/plans
  const resolvedServerName = useMemo(() => {
    if (serverName === 'content-store') return 'contentstore';
    return serverName;
  }, [serverName]);

  const loadServerProxy = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      logger.info('Proxy: Loading built-in tools for Rust MCP server', {
        serverName: resolvedServerName,
      });

      const tools = await listBuiltinTools(resolvedServerName);

      logger.info('Proxy: Built-in tools loaded successfully', {
        serverName: resolvedServerName,
        toolCount: tools.length,
        toolNames: tools.map((t) => t.name),
        toolDetails: tools.map((t) => ({
          name: t.name,
          description: t.description,
          inputSchema: t.inputSchema,
        })),
      });

      // Build dynamic proxy with tool methods
      const serverProxy: RustMCPServerProxy = {
        name: resolvedServerName,
        isLoaded: true,
        tools,
      };

      logger.info('Proxy: Initializing proxy object', {
        serverName: resolvedServerName,
        initialProxyKeys: Object.keys(serverProxy),
      });

      tools.forEach((tool) => {
        // Remove optional `${serverName}__` prefix if present
        const methodName = tool.name.startsWith(`${resolvedServerName}__`)
          ? tool.name.replace(`${resolvedServerName}__`, '')
          : tool.name;

        logger.info('Proxy: Attaching tool method to proxy', {
          serverName: resolvedServerName,
          toolName: tool.name,
          methodName,
          toolDescription: tool.description,
        });

        serverProxy[methodName] = async (
          args?: Record<string, unknown> | undefined,
        ) => {
          const safeArgs = args ?? {};
          logger.info('Proxy: Calling builtin tool', {
            serverName: resolvedServerName,
            methodName,
            toolName: tool.name,
            hasArgs: Object.keys(safeArgs).length > 0,
            argKeys: Object.keys(safeArgs),
          });

          const response: MCPResponse<unknown> = await callBuiltinTool(
            resolvedServerName,
            methodName,
            safeArgs,
          );

          if (response.error) {
            logger.error('Proxy: Tool execution failed', {
              serverName: resolvedServerName,
              methodName,
              error: response.error,
            });
            throw new Error(
              `MCP tool execution failed: ${methodName} - ${response.error.message} (code: ${response.error.code})`,
            );
          }

          if (response.result && 'structuredContent' in response.result) {
            const sc = (response.result as { structuredContent?: unknown })
              .structuredContent;
            if (typeof sc !== 'undefined') {
              logger.info('Proxy: Tool execution completed successfully', {
                serverName: resolvedServerName,
                methodName,
                hasResult: true,
                resultType: typeof sc,
              });
              return sc;
            }
          }

          // Handle other response formats (e.g., createStore returns 'store')
          if (response.result) {
            if ('store' in response.result) {
              const store = (response.result as { store?: unknown }).store;
              if (typeof store !== 'undefined') {
                logger.info(
                  'Proxy: Tool execution completed successfully (store format)',
                  {
                    serverName: resolvedServerName,
                    methodName,
                    hasResult: true,
                    resultType: typeof store,
                  },
                );
                return store;
              }
            }

            // Fallback: return the entire result if it has content
            if (Object.keys(response.result).length > 0) {
              logger.info(
                'Proxy: Tool execution completed successfully (fallback)',
                {
                  serverName: resolvedServerName,
                  methodName,
                  hasResult: true,
                  resultKeys: Object.keys(response.result),
                },
              );
              return response.result;
            }
          }

          logger.error('Proxy: Tool execution failed - no structured content', {
            serverName: resolvedServerName,
            methodName,
            responseKeys: response.result ? Object.keys(response.result) : [],
          });
          throw new Error(
            `MCP tool execution failed: ${methodName} - Server did not return structured content in the expected format`,
          );
        };

        logger.info('Proxy: Tool method attached successfully', {
          serverName: resolvedServerName,
          methodName,
          isMethodAttached: typeof serverProxy[methodName] === 'function',
        });
      });

      logger.info('Proxy: All tool methods attached to proxy', {
        serverName: resolvedServerName,
        totalMethodsAttached: Object.getOwnPropertyNames(serverProxy).filter(
          (key) =>
            typeof serverProxy[key] === 'function' &&
            key !== 'name' &&
            key !== 'isLoaded' &&
            key !== 'tools',
        ).length,
        finalProxyKeys: Object.keys(serverProxy),
        hasCreateStore: typeof serverProxy['createStore'] === 'function',
      });

      setServer(serverProxy as T);
      logger.info('Proxy: Rust MCP server proxy created successfully', {
        serverName: resolvedServerName,
        toolCount: tools.length,
        availableMethods: tools.map((t) =>
          t.name.startsWith(`${resolvedServerName}__`)
            ? t.name.replace(`${resolvedServerName}__`, '')
            : t.name,
        ),
        proxyKeys: Object.keys(serverProxy),
        proxyMethods: Object.getOwnPropertyNames(serverProxy).filter(
          (key) =>
            typeof serverProxy[key] === 'function' &&
            key !== 'name' &&
            key !== 'isLoaded' &&
            key !== 'tools',
        ),
      });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      logger.error('Proxy: Failed to load Rust MCP server tools', {
        serverName: resolvedServerName,
        error: errorMessage,
      });
    } finally {
      setLoading(false);
    }
  }, [resolvedServerName]);

  // Auto-load on mount or when server name changes
  useEffect(() => {
    if (!server) {
      void loadServerProxy();
    }
  }, [server, loadServerProxy]);

  return {
    server,
    loading,
    error,
    reload: loadServerProxy,
  } as const;
}

import { useCallback, useEffect, useState } from 'react';
import { getLogger } from '../lib/logger';
import { useWebMCP, WebMCPServerProxy } from '@/context/WebMCPContext';

const logger = getLogger('WebMCPServer');

/**
 * Re-exports the dynamic server proxy interface used by WebMCP.
 * See WebMCPContext for the full response contract and tool mapping rules.
 */
export type { WebMCPServerProxy } from '@/context/WebMCPContext';

/**
 * Hook: useWebMCPServer
 *
 * Purpose:
 * - Lazily obtain a dynamic proxy for a WebMCP server running in a Web Worker.
 * - The returned proxy exposes methods for each tool the server provides.
 *
 * Parameters:
 * - serverName: string — The MCP server module name (e.g., "content-store", "planning").
 *
 * Returns:
 * - { server, loading, error, reload }
 *   - server: T | null — Dynamic proxy with tool methods; null while loading/failed
 *   - loading: boolean — True while either context or proxy is loading
 *   - error: string | null — Last error message, if any
 *   - reload: () => Promise<void> — Force reload of the server proxy
 *
 * Response Contract:
 * When calling tool methods on the returned server proxy, the response follows this precedence:
 * 1. `result.structuredContent` (preferred) → returned as-is for typed data
 * 2. `result.content[0].text` → JSON.parse() attempted, falls back to raw string
 * 3. Raw `result` object → returned as fallback
 *
 * This enables both structured servers (content-store) and text-only servers (planning)
 * to work seamlessly with the same client interface.
 *
 * @example
 * ```typescript
 * // Structured response server (content-store)
 * const { server } = useWebMCPServer<ContentStoreServer>('content-store');
 * const result = await server.createStore({ metadata: { sessionId } });
 * // result => { storeId: string, createdAt: string } (typed)
 *
 * // Text-only server (planning)
 * const { server: planServer } = useWebMCPServer('planning');
 * const todo = await planServer.add_todo({ name: 'Write docs' });
 * // todo => string or parsed JSON
 *
 * // For both text and data, use toToolResult helper:
 * import { toToolResult } from '@/lib/web-mcp/tool-result';
 * const response = await proxy.callTool('server', 'tool', args);
 * const both = toToolResult<MyType>(response);
 * console.log(both.text, both.data);
 * ```
 *
 * Behavior:
 * - Automatically loads the server proxy on first render once context is initialized.
 * - Tool methods are dynamically created with normalized names (server prefix removed).
 * - All responses follow the consistent parsing contract for predictable behavior.
 */
export function useWebMCPServer<T extends WebMCPServerProxy>(
  serverName: string,
) {
  const [server, setServer] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { getServerProxy, isLoading: contextLoading } = useWebMCP();

  const loadServerProxy = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const serverProxy = await getServerProxy<T>(serverName);
      setServer(serverProxy);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      logger.error('Failed to load server proxy', { serverName, error: err });
    } finally {
      setLoading(false);
    }
  }, [serverName, getServerProxy]);

  // Auto-load server proxy
  useEffect(() => {
    if (!contextLoading && !server) {
      loadServerProxy();
    }
  }, [server, loadServerProxy, contextLoading]);

  return {
    server,
    loading: loading || contextLoading,
    error,
    reload: loadServerProxy,
  } as const;
}

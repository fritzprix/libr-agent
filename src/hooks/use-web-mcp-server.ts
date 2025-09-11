import { useCallback, useEffect, useState } from 'react';
import { getLogger } from '../lib/logger';
import { useWebMCP, WebMCPServerProxy } from '@/context/WebMCPContext';

const logger = getLogger('WebMCPServer');

// Export the interface from context
export type { WebMCPServerProxy } from '@/context/WebMCPContext';

// Simplified useWebMCPServer hook using context
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

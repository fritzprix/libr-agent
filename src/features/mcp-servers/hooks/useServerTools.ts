import { useState, useEffect } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp';

const logger = getLogger('useServerTools');

export function useServerTools(serverId: string, isOpen: boolean) {
  const [tools, setTools] = useState<MCPTool[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;

    setIsLoading(true);
    setError(null);
    setTools([]);

    safeInvoke<MCPTool[]>('probe_mcp_server', { serverId })
      .then((result) => {
        if (isMounted) setTools(result);
      })
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        logger.error('Failed to probe server tools', { serverId, err });
        if (isMounted) setError(msg);
      })
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [isOpen, serverId]);

  return { tools, isLoading, error };
}

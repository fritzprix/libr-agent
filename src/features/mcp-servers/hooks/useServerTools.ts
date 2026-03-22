import useSWR from 'swr';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp';

const logger = getLogger('useServerTools');

export function useServerTools(serverId: string, isOpen: boolean) {
  const { data, error, isLoading } = useSWR(
    isOpen ? ['server-tools', serverId] : null,
    async ([, id]) => {
      return await safeInvoke<MCPTool[]>('probe_mcp_server', { serverId: id });
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to probe server tools', { serverId, error: err });
      },
    },
  );

  return {
    tools: data || [],
    isLoading,
    error: error ? (error instanceof Error ? error.message : String(error)) : null,
  };
}

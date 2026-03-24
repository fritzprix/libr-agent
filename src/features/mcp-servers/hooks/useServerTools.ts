import useSWR from 'swr';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { MCPTool } from '@/lib/mcp';

const logger = getLogger('useServerTools');
const TOOL_NAME_SAMPLE_SIZE = 5;

function summarizeToolNames(tools: MCPTool[]): string {
  const preview = tools
    .slice(0, TOOL_NAME_SAMPLE_SIZE)
    .map((tool) => tool.name)
    .join(', ');

  if (tools.length > TOOL_NAME_SAMPLE_SIZE) {
    return `${preview} (+${tools.length - TOOL_NAME_SAMPLE_SIZE} more)`;
  }

  return preview;
}

export function useServerTools(serverId: string, isOpen: boolean) {
  const {
    data: tools = [],
    isLoading,
    error,
  } = useSWR<MCPTool[], Error>(
    isOpen ? ['probe-mcp-server', serverId] : null,
    async ([, id]) => {
      return await safeInvoke<MCPTool[]>('probe_mcp_server', { serverId: id });
    },
    {
      revalidateOnFocus: false,
      onSuccess: (result) => {
        logger.info('Loaded probed server tools', {
          serverId,
          toolCount: result.length,
          namesSample: summarizeToolNames(result),
        });
      },
      onError: (err) => {
        logger.error('Failed to probe server tools', { serverId, err });
      },
    },
  );

  return {
    tools,
    isLoading,
    error:
      error instanceof Error ? error.message : error ? String(error) : null,
  };
}

import { useMemo } from 'react';
import useSWR from 'swr';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import type { MCPTool } from '@/lib/mcp';
import { getLogger } from '@/lib/logger';
import { validateMCPTools } from '@/lib/schemas/mcp-tool';
import { isBuiltinTool } from '@/lib/tool-call-utils';

const logger = getLogger('useAgentTools');
const TOOL_NAME_SAMPLE_SIZE = 5;

export type McpToolsDiscoveryServer = {
  name: string;
  status: string;
  toolCount: number;
};

type AgentToolsKey = readonly ['agent-tools', string, string];

/**
 * Stable revision for MCP tool list cache invalidation.
 *
 * Loading UI tracks per-server runtime status live, but `agent-tools` SWR can
 * finish early (e.g. 10s soft timeout) before slow stdio servers register tools.
 * Including server status + tool counts forces a refetch when discovery catches up.
 */
export function buildMcpToolsDiscoveryRevision(
  servers: McpToolsDiscoveryServer[],
  proxyReady: boolean,
): string {
  const serverPart = [...servers]
    .map((server) => `${server.name}:${server.status}:${server.toolCount}`)
    .sort((a, b) => a.localeCompare(b))
    .join('|');
  return `${proxyReady ? 'ready' : 'pending'}:${serverPart}`;
}

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

export type UseAgentToolsOptions = {
  /**
   * Discovery revision from session runtime (proxy ready + per-server status).
   * When omitted, tools are fetched once per session without discovery refreshes.
   */
  discoveryRevision?: string;
};

/**
 * Hook to fetch available tools for a specific agent session
 * Returns the filtered tool list based on agent configuration
 * Ensures UI displays the same tools that LLM can actually use
 *
 * @param sessionId - The active agent session ID
 * @param options - Optional discovery revision for cache invalidation
 * @returns Object containing availableTools, isLoading, and error
 */
export function useAgentTools(
  sessionId: string | undefined,
  options?: UseAgentToolsOptions,
) {
  const discoveryRevision = options?.discoveryRevision ?? 'static';

  const swrKey: AgentToolsKey | null = useMemo(
    () => (sessionId ? ['agent-tools', sessionId, discoveryRevision] : null),
    [sessionId, discoveryRevision],
  );

  const {
    data: availableTools = [],
    isLoading,
    error,
  } = useSWR<MCPTool[], Error, AgentToolsKey | null>(
    swrKey,
    async ([, id]) => {
      logger.debug('Loading agent tools', {
        sessionId: id,
        discoveryRevision,
      });

      const response = await getAgentAvailableTools(id);

      if (!Array.isArray(response)) {
        throw new Error('Expected array of tools from backend');
      }

      const tools = validateMCPTools(response);

      if (tools.length !== response.length) {
        logger.warn('Some tools failed validation', {
          sessionId: id,
          received: response.length,
          validated: tools.length,
        });
      }

      return tools;
    },
    {
      revalidateOnFocus: false,
      keepPreviousData: true,
      onSuccess: (tools, key) => {
        const externalTools = tools.filter((tool) => !isBuiltinTool(tool.name));
        logger.info('Loaded agent tools', {
          sessionId: key[1],
          discoveryRevision: key[2],
          toolCount: tools.length,
          builtinCount: tools.length - externalTools.length,
          externalCount: externalTools.length,
          externalNamesSample: summarizeToolNames(externalTools),
        });
      },
      onError: (err, key) => {
        logger.error('Failed to load agent tools', {
          sessionId: key[1],
          discoveryRevision: key[2],
          error: err,
        });
      },
    },
  );

  return {
    availableTools,
    isLoading,
    error:
      error instanceof Error
        ? error.message
        : error
          ? String(error)
          : undefined,
  };
}

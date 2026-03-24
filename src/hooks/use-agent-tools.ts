import useSWR from 'swr';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import type { MCPTool } from '@/lib/mcp';
import { getLogger } from '@/lib/logger';
import { validateMCPTools } from '@/lib/schemas/mcp-tool';
import { isBuiltinTool } from '@/lib/tool-call-utils';

const logger = getLogger('useAgentTools');
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

/**
 * Hook to fetch available tools for a specific agent session
 * Returns the filtered tool list based on agent configuration
 * Ensures UI displays the same tools that LLM can actually use
 *
 * @param sessionId - The active agent session ID
 * @returns Object containing availableTools, isLoading, and error
 */
export function useAgentTools(sessionId: string | undefined) {
  const {
    data: availableTools = [],
    isLoading,
    error,
  } = useSWR<MCPTool[], Error>(
    sessionId ? ['agent-tools', sessionId] : null,
    async ([, id]) => {
      logger.debug('Loading agent tools', { sessionId: id });

      const response = await getAgentAvailableTools(id as string);

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
      onSuccess: (tools) => {
        const externalTools = tools.filter((tool) => !isBuiltinTool(tool.name));
        logger.info('Loaded agent tools', {
          sessionId,
          toolCount: tools.length,
          builtinCount: tools.filter((t) => isBuiltinTool(t.name)).length,
          externalCount: externalTools.length,
          externalNamesSample: summarizeToolNames(externalTools),
        });
      },
      onError: (err) => {
        logger.error('Failed to load agent tools', { sessionId, error: err });
      },
    }
  );

  return {
    availableTools,
    isLoading,
    error: error instanceof Error ? error.message : error ? String(error) : undefined,
  };
}

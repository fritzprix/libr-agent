import { useEffect, useState } from 'react';
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
  const [availableTools, setAvailableTools] = useState<MCPTool[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    if (!sessionId) {
      setAvailableTools([]);
      setIsLoading(false);
      setError(undefined);
      return;
    }

    let isMounted = true;

    const loadTools = async () => {
      setIsLoading(true);
      setError(undefined);

      try {
        logger.debug('Loading agent tools', { sessionId });

        const response = await getAgentAvailableTools(sessionId);

        // Validate response is an array
        if (!Array.isArray(response)) {
          throw new Error('Expected array of tools from backend');
        }

        // Filter and validate tools using Zod schema
        const tools = validateMCPTools(response);

        if (tools.length !== response.length) {
          logger.warn('Some tools failed validation', {
            sessionId,
            received: response.length,
            validated: tools.length,
          });
        }

        if (isMounted) {
          setAvailableTools(tools);
          const externalTools = tools.filter(
            (tool) => !isBuiltinTool(tool.name),
          );
          logger.info('Loaded agent tools', {
            sessionId,
            toolCount: tools.length,
            builtinCount: tools.filter((t) => isBuiltinTool(t.name)).length,
            externalCount: externalTools.length,
            externalNamesSample: summarizeToolNames(externalTools),
          });
        }
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        if (isMounted) {
          setError(errorMessage);
          setAvailableTools([]);
          logger.error('Failed to load agent tools', { sessionId, error: err });
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    };

    loadTools();

    return () => {
      isMounted = false;
    };
  }, [sessionId]);

  return { availableTools, isLoading, error };
}

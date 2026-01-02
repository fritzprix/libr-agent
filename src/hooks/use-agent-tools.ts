import { useEffect, useState } from 'react';
import { getAgentAvailableTools } from '@/lib/rust-backend-client';
import type { MCPTool } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAgentTools');

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

        const tools = (await getAgentAvailableTools(sessionId)) as MCPTool[];

        if (isMounted) {
          setAvailableTools(tools);
          logger.info('Loaded agent tools', {
            sessionId,
            toolCount: tools.length,
            builtinCount: tools.filter((t) => t.name.startsWith('builtin_'))
              .length,
            externalCount: tools.filter((t) => !t.name.startsWith('builtin_'))
              .length,
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

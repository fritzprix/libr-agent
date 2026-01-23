import { useCallback } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { createToolMessagePair } from '@/lib/chat-utils';
import { stringToMCPContentArray } from '@/lib/utils';
import { getLogger } from '@/lib/logger';
import { MCPContent } from '@/lib/mcp-types';

const logger = getLogger('useAgentToolExecution');

/**
 * Hook to execute an agent tool and inject the result into the chat stream.
 * Encapsulates the pattern of:
 * 1. Calling the backend tool
 * 2. Creating tool call/result message pair
 * 3. Injecting messages into the session to trigger workflow
 */
export function useAgentToolExecution() {
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { agentCallBuiltinTool } = useRustBackend();

  const executeTool = useCallback(async (
    toolName: string,
    args: Record<string, unknown>,
    options?: {
      resultType?: 'text' | 'ui';
      triggerWorkflow?: boolean;
    }
  ) => {
    if (!session?.id) {
      const msg = 'Cannot execute tool: no active session';
      logger.error(msg);
      throw new Error(msg);
    }

    try {
      logger.info('Executing tool', { toolName, args });

      const response = await agentCallBuiltinTool(
        session.id,
        toolName,
        args
      );

      let content: MCPContent[] = [];

      // Handle error responses
      if (response.isError === true) {
        let errorText = 'Tool execution failed';
        const firstItem = response.content?.[0];

        // Try to extract text from error content
        if (firstItem) {
          if ('text' in firstItem && typeof firstItem.text === 'string') {
            errorText = firstItem.text;
          } else {
             errorText = JSON.stringify(firstItem);
          }
        }

        content = stringToMCPContentArray(errorText);
      } else {
        // Use content directly if available, otherwise default empty
        content = (response.content as MCPContent[]) || [];

        // Fallback for missing content
        if (content.length === 0) {
          content = stringToMCPContentArray('No result returned');
        }
      }

      const toolCallId = createId();

      const [toolCallMessage, toolResultMessage] = createToolMessagePair(
        toolName,
        args,
        content,
        toolCallId,
        session.id,
        undefined,
        session.assistant?.id,
        options?.resultType
      );

      // Default to true for triggerWorkflow if not specified
      const shouldTrigger = options?.triggerWorkflow ?? true;
      await injectMessages([toolCallMessage, toolResultMessage], shouldTrigger);

      return response;
    } catch (error) {
      logger.error('Failed to execute tool', { toolName, error });
      throw error;
    }
  }, [session, injectMessages, agentCallBuiltinTool]);

  return { executeTool };
}

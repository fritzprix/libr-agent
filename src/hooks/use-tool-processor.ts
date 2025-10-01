import { useCallback, useRef, useTransition, useEffect } from 'react';
import { useAsyncFn } from 'react-use';
import { useSessionContext } from '../context/SessionContext';
import { useAssistantContext } from '../context/AssistantContext';
import { useUnifiedMCP } from './use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message, ToolCall } from '@/models/chat';
import { isMCPError, MCPContent, MCPResponse } from '@/lib/mcp-types';
import { useSessionHistory } from '@/context/SessionHistoryContext';
import { extractBuiltInServiceAlias } from '@/lib/utils';

const logger = getLogger('useToolProcessor');

interface UseToolProcessorConfig {
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
}

const buildErrorContent = (text: string): MCPContent[] => {
  return [{ type: 'text', text }];
};

// Tool Call ID 검증 강화
const fixInvalidToolCall = (toolCall: ToolCall): ToolCall => {
  // Tool call ID가 유효한지 검증
  if (!toolCall.id || toolCall.id.trim().length === 0) {
    return { ...toolCall, id: createId() }; // 유효하지 않으면 새로운 ID 생성
  }
  return toolCall;
};

const hasUIResource = (message: MCPResponse<unknown>): boolean => {
  return (
    message.result?.content?.some((m: MCPContent) => m.type === 'resource') ||
    false
  );
};

export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { executeToolCall } = useUnifiedMCP();
  const { addMessages } = useSessionHistory();

  const lastProcessedMessageId = useRef<string | null>(null);
  const [isPending, startTransition] = useTransition();

  // Use refs for stable references to prevent unnecessary re-renders
  const submitRef = useRef(submit);
  const executeToolCallRef = useRef(executeToolCall);

  // Update refs when dependencies change
  useEffect(() => {
    submitRef.current = submit;
    executeToolCallRef.current = executeToolCall;
  });

  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      if (!tcMessage.tool_calls || tcMessage.tool_calls.length === 0) {
        logger.warn('No tool calls found in message');
        return;
      }

      try {
        logger.info('Starting tool execution batch', {
          toolCallCount: tcMessage.tool_calls.length,
          messageId: tcMessage.id,
        });

        const toolPromises = tcMessage.tool_calls
          .map(fixInvalidToolCall)
          .map(async (toolCall) => {
            const toolName = toolCall.function.name;
            const executionStartTime = Date.now();

            try {
              // Runtime security validation for built-in tools
              if (toolName.startsWith('builtin_')) {
                const alias = extractBuiltInServiceAlias(toolName);
                const allowedAliases =
                  currentAssistant?.allowedBuiltInServiceAliases;

                // If allowedAliases is defined, enforce the restrictions
                if (allowedAliases !== undefined) {
                  const isAllowed =
                    !!alias && allowedAliases.includes(alias);

                  if (!isAllowed) {
                    const errorMsg = `Tool ${toolName} is not allowed for assistant "${currentAssistant?.name}"`;
                    logger.warn('Tool execution blocked', {
                      toolName,
                      alias,
                      allowedAliases,
                      assistant: currentAssistant?.name,
                    });
                    throw new Error(errorMsg);
                  }
                }
              }

              logger.debug('Executing tool', {
                toolName,
                toolCallId: toolCall.id,
              });

              const mcpResponse = await executeToolCallRef.current(toolCall);
              const executionTime = Date.now() - executionStartTime;

              // Diagnostic logging for debugging readContent tool result loss
              logger.info('Raw mcpResponse for tool', {
                toolCallId: toolCall.id,
                toolName,
                mcpResponse,
              });

              const toolResultMessage: Message = {
                id: createId(),
                assistantId: currentAssistant?.id,
                role: 'tool',
                content: isMCPError(mcpResponse)
                  ? buildErrorContent(
                      `Error: ${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`,
                    )
                  : mcpResponse.result?.content || [],
                tool_call_id: toolCall.id,
                sessionId: currentSession?.id || '',
              };

              const hasUi = hasUIResource(mcpResponse);

              logger.info('Tool execution completed', {
                toolName,
                success: !isMCPError(mcpResponse),
                executionTime,
              });

              return { message: toolResultMessage, hasUi };
            } catch (error) {
              logger.error('Tool execution failed', { toolName, error });

              const errorMessage: Message = {
                id: createId(),
                assistantId: currentAssistant?.id,
                role: 'tool',
                content: buildErrorContent(
                  `Error executing ${toolName}: ${error instanceof Error ? error.message : 'Unknown error'}`,
                ),
                sessionId: currentSession?.id || '',
                tool_call_id: toolCall.id,
              };

              return { message: errorMessage, hasUi: false };
            }
          });

        // Wait for all tool calls to complete in parallel
        const toolResults = await Promise.all(toolPromises);
        const messages = toolResults.map((result) => result.message);

        if (messages.length > 0) {
          const hasUIResults = toolResults.some((result) => result.hasUi);
          if (!hasUIResults) {
            logger.info('Submitting tool results', {
              resultCount: messages.length,
              messageId: tcMessage.id,
            });
            submitRef.current(messages, currentAssistant?.id);
          } else {
            addMessages(messages);
          }
        }
      } catch (e) {
        logger.error('error', e);
      }
    },
    [], // Empty dependency array for stable reference
  );

  const processToolCalls = useCallback(
    (message: Message) => {
      if (
        message &&
        message.role === 'assistant' &&
        message.tool_calls &&
        message.tool_calls.length > 0 &&
        !message.isStreaming &&
        !loading &&
        !isPending &&
        message.id &&
        lastProcessedMessageId.current !== message.id
      ) {
        lastProcessedMessageId.current = message.id;
        // Use startTransition to make tool execution non-blocking for UI
        startTransition(() => {
          execute(message);
        });
      }
    },
    [execute, loading, isPending, startTransition],
  );

  return {
    processToolCalls,
    isProcessing: loading || isPending,
  };
};

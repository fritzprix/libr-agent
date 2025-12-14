import { useCallback, useRef, useTransition, useEffect } from 'react';
import { useAsyncFn } from 'react-use';
import { useSessionContext } from '../context/SessionContext';
import { useAssistantContext } from '../context/AssistantContext';
import { useUnifiedMCP } from './use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message, ToolCall, MessageErrorType } from '@/models/chat';
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

// Enhanced Tool Call ID validation
const fixInvalidToolCall = (toolCall: ToolCall): ToolCall => {
  // Validate if tool call ID is valid
  if (!toolCall.id || toolCall.id.trim().length === 0) {
    return { ...toolCall, id: createId() }; // Generate new ID if invalid
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

        const toolResults: { message: Message; hasUi: boolean }[] = [];

        // Sequential execution of tool calls
        for (const toolCall of tcMessage.tool_calls.map(fixInvalidToolCall)) {
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
                const isAllowed = !!alias && allowedAliases.includes(alias);

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
              hasResult: !!mcpResponse.result,
              hasError: !!mcpResponse.error,
              contentCount: mcpResponse.result?.content?.length || 0,
              contentTypes:
                mcpResponse.result?.content?.map((c: MCPContent) => c.type) ||
                [],
            });

            // Detect both protocol-level and tool execution errors
            const hasProtocolError = isMCPError(mcpResponse);
            const hasToolExecutionError = mcpResponse.result?.isError === true;
            const hasAnyError = hasProtocolError || hasToolExecutionError;

            // Extract appropriate error message
            const errorMessage = hasProtocolError
              ? `Error: ${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`
              : hasToolExecutionError
                ? ((mcpResponse.result?.content?.[0] as { text?: string })
                    ?.text ?? 'Unknown error')
                : '';

            const toolResultMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              content: hasAnyError
                ? buildErrorContent(errorMessage)
                : mcpResponse.result?.content || [],
              tool_call_id: toolCall.id,
              sessionId: currentSession?.id || '',
              threadId: currentSession?.id || '', // Default to top thread
              metadata: {
                executionTime,
              },
              // Map both protocol errors and tool execution errors to Message.error
              ...(hasAnyError && {
                error: {
                  displayMessage: hasProtocolError
                    ? mcpResponse.error.message
                    : errorMessage,
                  type: hasProtocolError
                    ? ('MCP_ERROR' as MessageErrorType)
                    : ('TOOL_EXECUTION_ERROR' as MessageErrorType),
                  recoverable: true,
                  details: {
                    originalError: hasProtocolError
                      ? mcpResponse.error
                      : {
                          isError: true,
                          content: mcpResponse.result?.content,
                        },
                    errorCode: hasProtocolError
                      ? `MCP_${mcpResponse.error.code}`
                      : 'TOOL_ERROR',
                    timestamp: new Date().toISOString(),
                    context: {
                      toolName,
                      toolCallId: toolCall.id,
                      isToolExecutionError: hasToolExecutionError,
                    },
                  },
                },
              }),
            };

            const hasUi = hasUIResource(mcpResponse);

            logger.info('Tool execution completed', {
              toolName,
              success: !hasAnyError,
              hasProtocolError,
              hasToolExecutionError,
              executionTime,
            });

            toolResults.push({ message: toolResultMessage, hasUi });
          } catch (error) {
            const executionTime = Date.now() - executionStartTime;
            logger.error('Tool execution failed', { toolName, error });

            const errorMsg =
              error instanceof Error ? error.message : 'Unknown error';

            const errorMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              content: buildErrorContent(
                `Error executing ${toolName}: ${errorMsg}`,
              ),
              sessionId: currentSession?.id || '',
              threadId: currentSession?.id || '', // Default to top thread
              tool_call_id: toolCall.id,
              metadata: {
                executionTime,
              },
              // Structured error for type-safe error detection
              error: {
                displayMessage: `Error executing ${toolName}`,
                type: 'TOOL_EXECUTION_ERROR' as MessageErrorType,
                recoverable: true,
                details: {
                  originalError: error,
                  timestamp: new Date().toISOString(),
                  context: {
                    toolName,
                    toolCallId: toolCall.id,
                  },
                },
              },
            };

            toolResults.push({ message: errorMessage, hasUi: false });
          }
        }
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

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

  // State for tracking history
  const toolHistoryRef = useRef<{ signature: string; count: number } | null>(
    null,
  );

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
          const args = toolCall.function.arguments;

          // Circuit Breaker Logic
          const currentSignature = `${toolName}:${args}`;
          let isLooping = false;

          if (toolHistoryRef.current?.signature === currentSignature) {
            toolHistoryRef.current.count += 1;
            if (toolHistoryRef.current.count >= 3) {
              isLooping = true;
            }
          } else {
            toolHistoryRef.current = { signature: currentSignature, count: 1 };
          }

          const executionStartTime = Date.now();

          try {
            let mcpResponse: MCPResponse<unknown>;

            if (isLooping) {
              logger.warn('Circuit breaker triggered', {
                toolName,
                count: toolHistoryRef.current?.count,
              });

              const circuitBreakCall = {
                ...toolCall,
                function: {
                  name: 'builtin_ui__circuitBreak',
                  arguments: JSON.stringify({
                    toolName,
                    repetitionCount: toolHistoryRef.current?.count,
                    args,
                  }),
                },
              };

              mcpResponse = await executeToolCallRef.current(circuitBreakCall);
              // Reset history after triggering to avoid infinite blocking if user resumes
              toolHistoryRef.current = null;
            } else {
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

              mcpResponse = await executeToolCallRef.current(toolCall);
            }

            const finalMcpResponse = mcpResponse;
            const executionTime = Date.now() - executionStartTime;

            // Detect both protocol-level and tool execution errors
            const hasProtocolError = isMCPError(finalMcpResponse);
            const hasToolExecutionError =
              finalMcpResponse.result?.isError === true;
            const hasAnyError = hasProtocolError || hasToolExecutionError;

            // Extract appropriate error message
            const errorMessage = hasProtocolError
              ? `Error: ${finalMcpResponse.error.message} (Code: ${finalMcpResponse.error.code})`
              : hasToolExecutionError
                ? ((finalMcpResponse.result?.content?.[0] as { text?: string })
                    ?.text ?? 'Unknown error')
                : '';

            const toolResultMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              // Preserve original content even for errors (don't replace)
              content:
                finalMcpResponse.result?.content ||
                buildErrorContent(errorMessage),
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
                    ? finalMcpResponse.error.message
                    : errorMessage,
                  type: hasProtocolError
                    ? ('MCP_ERROR' as MessageErrorType)
                    : ('TOOL_EXECUTION_ERROR' as MessageErrorType),
                  recoverable: true,
                  details: {
                    originalError: hasProtocolError
                      ? finalMcpResponse.error
                      : {
                          isError: true,
                          content: finalMcpResponse.result?.content,
                        },
                    errorCode: hasProtocolError
                      ? `MCP_${finalMcpResponse.error.code}`
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

            const hasUi = hasUIResource(finalMcpResponse);

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

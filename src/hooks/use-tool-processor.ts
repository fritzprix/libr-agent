import { useCallback, useRef, useTransition, useEffect } from 'react';
import { useAsyncFn } from 'react-use';
import { useSessionContext } from '../context/SessionContext';
import { useAssistantContext } from '../context/AssistantContext';
import { useUnifiedMCP } from './use-unified-mcp';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '../lib/logger';
import { Message } from '@/models/chat';
import { isMCPError } from '@/lib/mcp-types';

const logger = getLogger('useToolProcessor');

interface UseToolProcessorConfig {
  onToolExecutionChange: (isExecuting: boolean) => void;
  submit: (messageToAdd?: Message[], agentKey?: string) => Promise<Message>;
}

export const useToolProcessor = ({ onToolExecutionChange, submit }: UseToolProcessorConfig) => {
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { executeToolCall } = useUnifiedMCP();

  const lastProcessedMessageId = useRef<string | null>(null);
  const [isPending, startTransition] = useTransition();

  // Use refs for stable references to prevent unnecessary re-renders
  const submitRef = useRef(submit);
  const executeToolCallRef = useRef(executeToolCall);
  const onToolExecutionChangeRef = useRef(onToolExecutionChange);

  // Update refs when dependencies change
  useEffect(() => {
    submitRef.current = submit;
    executeToolCallRef.current = executeToolCall;
    onToolExecutionChangeRef.current = onToolExecutionChange;
  });

  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      if (!tcMessage.tool_calls || tcMessage.tool_calls.length === 0) {
        logger.warn('No tool calls found in message');
        return;
      }

      // Tool 실행 시작을 부모에게 알림
      onToolExecutionChangeRef.current(true);

      try {
        logger.info('Starting tool execution batch', {
          toolCallCount: tcMessage.tool_calls.length,
          messageId: tcMessage.id,
        });

        // Tool Call ID 검증 강화
        const validateToolCallId = (toolCall: { id: string }): boolean => {
          // Tool call ID가 유효한지 검증
          return Boolean(toolCall.id && toolCall.id.trim().length > 0);
        };

        // Tool 실행 전 검증 로직 추가
        for (const toolCall of tcMessage.tool_calls) {
          if (!validateToolCallId(toolCall)) {
            logger.error('Invalid tool call ID detected', { toolCall });
            // 유효하지 않은 ID의 경우 새로운 deterministic ID 생성
            toolCall.id = `fallback_${Math.abs(
              JSON.stringify(toolCall.function)
                .split('')
                .reduce((a, b) => {
                  a = (a << 5) - a + b.charCodeAt(0);
                  return a & a;
                }, 0),
            ).toString(36)}`;
          }
        }

        // Execute all tool calls in parallel for better performance
        const toolPromises = tcMessage.tool_calls.map(async (toolCall) => {
          const toolName = toolCall.function.name;
          const executionStartTime = Date.now();

          try {
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
                ? `Error: ${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`
                : mcpResponse.result?.content || '',
              tool_call_id: toolCall.id,
              sessionId: currentSession?.id || '',
            };

            logger.info('Tool execution completed', {
              toolName,
              success: !isMCPError(mcpResponse),
              executionTime,
            });

            return toolResultMessage;
          } catch (error) {
            logger.error('Tool execution failed', { toolName, error });

            const errorMessage: Message = {
              id: createId(),
              assistantId: currentAssistant?.id,
              role: 'tool',
              content: `Error executing ${toolName}: ${error instanceof Error ? error.message : 'Unknown error'}`,
              sessionId: currentSession?.id || '',
              tool_call_id: toolCall.id,
            };

            return errorMessage;
          }
        });

        // Wait for all tool calls to complete in parallel
        const toolResults = await Promise.all(toolPromises);

        if (toolResults.length > 0) {
          logger.info('Submitting tool results', {
            resultCount: toolResults.length,
            messageId: tcMessage.id,
          });

          await submitRef.current(toolResults, currentAssistant?.id);
        }
      } finally {
        // Tool 실행 완료를 부모에게 알림
        onToolExecutionChangeRef.current(false);
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
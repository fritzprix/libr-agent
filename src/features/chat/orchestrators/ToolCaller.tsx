import { useAssistantContext } from '@/context/AssistantContext';
import { useLocalTools } from '@/context/LocalToolContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/hooks/use-chat';
import { useMCPServer } from '@/hooks/use-mcp-server';
import {
  isMCPError,
  isMCPSuccess,
  MCPResponse,
  mcpResponseToString,
} from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useCallback, useEffect, useRef } from 'react';
import { useAsyncFn } from 'react-use';

const logger = getLogger('ToolCaller');

interface SerializedToolResult {
  success: boolean;
  content?: string;
  error?: string;
  metadata: Record<string, unknown>;
  toolName: string;
  executionTime: number;
  timestamp: string;
}

export const ToolCaller: React.FC = () => {
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { messages, submit } = useChatContext();
  const { executeToolCall: callMcpTool } = useMCPServer();
  const { isLocalTool, executeToolCall: callLocalTool } = useLocalTools();
  const { schedule } = useScheduler();

  const serializeToolResult = useCallback(
    (
      mcpResponse: MCPResponse,
      toolName: string,
      executionStartTime: number,
    ): string => {
      // 추가 에러 검증: content에 에러가 있는지 확인
      const isContentError =
        isMCPSuccess(mcpResponse) &&
        mcpResponse.result?.content?.some(
          (content) =>
            content.type === 'text' &&
            (content.text.includes('"error":') ||
              content.text.includes('\\"error\\":')),
        );

      const actualSuccess = isMCPSuccess(mcpResponse) && !isContentError;

      let errorMessage: string | undefined;
      if (isMCPError(mcpResponse)) {
        errorMessage = mcpResponse.error.message;
      } else if (
        isContentError &&
        mcpResponse.result?.content?.[0]?.type === 'text'
      ) {
        // content에서 에러 메시지 추출
        try {
          const contentText = mcpResponse.result.content[0].text;
          const parsed = JSON.parse(contentText);
          errorMessage = parsed.error || contentText;
        } catch {
          errorMessage = mcpResponse.result.content[0].text;
        }
      }

      const result: SerializedToolResult = {
        success: actualSuccess,
        content: actualSuccess ? mcpResponseToString(mcpResponse) : undefined,
        error: errorMessage,
        metadata: {
          toolName,
          mcpResponseId: mcpResponse.id,
          jsonrpc: mcpResponse.jsonrpc,
          isValidated: true,
          hasContentError: isContentError,
        },
        toolName,
        executionTime: Date.now() - executionStartTime,
        timestamp: new Date().toISOString(),
      };

      return JSON.stringify(result);
    },
    [],
  );

  const lastProcessedMessageId = useRef<string | null>(null);

  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      if (!tcMessage.tool_calls || tcMessage.tool_calls.length === 0) {
        logger.warn('No tool calls found in message');
        return;
      }

      const toolResults: Message[] = [];

      for (const toolCall of tcMessage.tool_calls) {
        const toolName = toolCall.function.name;
        const executionStartTime = Date.now();

        try {
          logger.info(`Executing tool: ${toolName}`, {
            toolCall: toolCall.function.name,
            args: toolCall.function.arguments,
          });

          const callFunction = isLocalTool(toolName)
            ? callLocalTool
            : callMcpTool;
          const mcpResponse: MCPResponse = await callFunction(toolCall);

          const serializedContent = serializeToolResult(
            mcpResponse,
            toolName,
            executionStartTime,
          );

          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool',
            content: serializedContent,
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });

          if (isMCPSuccess(mcpResponse)) {
            logger.info(`Tool executed successfully: ${toolName}`);
          } else {
            logger.warn(`Tool execution finished with error: ${toolName}`, {
              error: (mcpResponse as MCPResponse & { error: object }).error,
            });
          }
        } catch (error) {
          logger.error(`Tool execution failed for ${toolName}:`, error);

          const errorResponse: MCPResponse = {
            jsonrpc: '2.0',
            id: `tool-${toolName}-${Date.now()}`,
            error: {
              code: -32000,
              message:
                error instanceof Error
                  ? error.message
                  : 'Unknown execution error',
              data: {
                errorType:
                  error instanceof Error
                    ? error.constructor.name
                    : 'UnknownError',
                toolCallId: toolCall.id,
              },
            },
          };

          const serializedContent = serializeToolResult(
            errorResponse,
            toolName,
            executionStartTime,
          );

          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool',
            content: serializedContent,
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });
        }
      }

      if (toolResults.length > 0) {
        await submit(toolResults);
      }
    },
    [
      submit,
      callLocalTool,
      callMcpTool,
      schedule,
      currentAssistant,
      currentSession,
      serializeToolResult,
    ],
  );

  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (
      lastMessage &&
      lastMessage.role === 'assistant' &&
      lastMessage.tool_calls &&
      lastMessage.tool_calls.length > 0 &&
      !lastMessage.isStreaming &&
      !loading &&
      lastMessage.id &&
      lastProcessedMessageId.current !== lastMessage.id
    ) {
      lastProcessedMessageId.current = lastMessage.id;
      execute(lastMessage);
    }
  }, [messages, execute, loading]);

  return null;
};

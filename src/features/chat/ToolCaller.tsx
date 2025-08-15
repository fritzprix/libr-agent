import { useAssistantContext } from '@/context/AssistantContext';
import { useChatContext } from '@/context/ChatContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useSessionContext } from '@/context/SessionContext';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import { getLogger } from '@/lib/logger';
import {
  isMCPError,
  MCPResponse,
  mcpResponseToString,
  MCPResourceContent,
} from '@/lib/mcp-types';
import { Message, UIResource } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useCallback, useEffect, useRef } from 'react';
import { useAsyncFn } from 'react-use';

const logger = getLogger('ToolCaller');

interface SerializedToolResult {
  success: boolean;
  text?: string;
  uiResource?: UIResource | UIResource[];
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
  const { executeToolCall } = useUnifiedMCP();

  const { schedule } = useScheduler();

  const serializeToolResult = useCallback(
    (
      mcpResponse: MCPResponse,
      toolName: string,
      executionStartTime: number,
    ): SerializedToolResult => {
      const executionTime = Date.now() - executionStartTime;
      const timestamp = new Date().toISOString();

      // 표준 MCP 에러 처리
      if (isMCPError(mcpResponse)) {
        return {
          success: false,
          error: `${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`,
          metadata: {
            toolName,
            mcpResponseId: mcpResponse.id,
            jsonrpc: mcpResponse.jsonrpc,
            errorCode: mcpResponse.error.code,
            errorData: mcpResponse.error.data,
          },
          toolName,
          executionTime,
          timestamp,
        };
      }

      // Tool 실행 에러 처리 (isError: true)
      if (mcpResponse.result?.isError) {
        const errorText =
          mcpResponse.result.content?.[0]?.type === 'text'
            ? mcpResponse.result.content[0].text
            : 'Tool execution failed';

        return {
          success: false,
          error: errorText,
          metadata: {
            toolName,
            mcpResponseId: mcpResponse.id,
            jsonrpc: mcpResponse.jsonrpc,
            isToolExecutionError: true,
          },
          toolName,
          executionTime,
          timestamp,
        };
      }

      // 성공 케이스 - UIResource 감지 및 보존
      let uiResources: UIResource[] = [];
      let textContent = '';

      if (mcpResponse.result?.content) {
        // UIResource 항목들을 찾아서 수집
        const resourceItems = mcpResponse.result.content.filter(
          (item): item is MCPResourceContent => item.type === 'resource',
        );

        if (resourceItems.length > 0) {
          uiResources = resourceItems.map((item) => item.resource);
          logger.info(
            `Found ${uiResources.length} UI resources in tool response`,
            {
              toolName,
              resourceTypes: uiResources.map((r) => r.mimeType),
            },
          );
        }

        // 텍스트 내용 수집 - UI Resource가 있으면 요약, 없으면 전체 텍스트
        if (uiResources.length > 0) {
          // UIResource가 있을 때는 간단한 요약만 제공 (HTML 등 대용량 컨텐츠 제외)
          const resourceSummary = uiResources
            .map((r) => `${r.mimeType}${r.uri ? ` (${r.uri})` : ''}`)
            .join(', ');
          textContent = `UI Resources: ${resourceSummary}`;
        } else {
          // UIResource가 없을 때는 기존 동작 유지
          textContent = mcpResponseToString(mcpResponse);
        }
      }

      return {
        success: true,
        text: textContent,
        uiResource:
          uiResources.length === 1
            ? uiResources[0]
            : uiResources.length > 1
              ? uiResources
              : undefined,
        metadata: {
          toolName,
          mcpResponseId: mcpResponse.id,
          jsonrpc: mcpResponse.jsonrpc,
          hasUIResource: uiResources.length > 0,
        },
        toolName,
        executionTime,
        timestamp,
      };
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

      logger.info('Starting tool execution batch', {
        toolCallCount: tcMessage.tool_calls.length,
      });

      const toolResults: Message[] = [];

      for (const toolCall of tcMessage.tool_calls) {
        const toolName = toolCall.function.name;
        const executionStartTime = Date.now();

        try {
          logger.info(`Executing tool: ${toolName}`, {
            toolCall: toolCall.function.name,
            args: toolCall.function.arguments,
          });

          // Use unified MCP system for all tools (Tauri MCP + Web MCP)
          const mcpResponse = await executeToolCall(toolCall);

          // 에러 처리: MCP 에러나 도구 실행 에러는 예외로 처리
          if (isMCPError(mcpResponse)) {
            throw new Error(
              `MCP Protocol Error: ${mcpResponse.error.message} (Code: ${mcpResponse.error.code})`,
            );
          }

          if (mcpResponse.result?.isError) {
            const errorText =
              mcpResponse.result.content?.[0]?.type === 'text'
                ? mcpResponse.result.content[0].text
                : 'Tool execution failed';
            throw new Error(`Tool Execution Error: ${errorText}`);
          }

          // 성공 케이스만 직렬화
          const serialized = serializeToolResult(
            mcpResponse,
            toolName,
            executionStartTime,
          );

          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool',
            content:
              serialized.text ||
              (serialized.uiResource
                ? `[UIResource: ${
                    Array.isArray(serialized.uiResource)
                      ? serialized.uiResource.length + ' resources'
                      : serialized.uiResource.uri || 'resource'
                  }]`
                : ''),
            uiResource: serialized.uiResource,
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });

          logger.info(`Tool executed successfully: ${toolName}`);
        } catch (error) {
          logger.error(`Tool execution failed for ${toolName}:`, error);

          // 에러 상황에서는 상세한 에러 정보를 포함한 메시지 생성
          const errorMessage =
            error instanceof Error ? error.message : 'Unknown execution error';
          const errorDetails = {
            toolName,
            error: errorMessage,
            toolCallId: toolCall.id,
            timestamp: new Date().toISOString(),
            executionTime: Date.now() - executionStartTime,
            errorType:
              error instanceof Error ? error.constructor.name : 'UnknownError',
          };

          // 에러 메시지를 사용자가 이해할 수 있는 형태로 구성
          const userFriendlyError = `❌ **Tool Execution Failed**

**Tool**: ${toolName}
**Error**: ${errorMessage}
**Time**: ${new Date().toLocaleTimeString()}
**Duration**: ${Date.now() - executionStartTime}ms

Please check the tool configuration or try again.`;

          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool', // 에러도 tool role로 유지하되, 내용으로 구분
            content: JSON.stringify({
              success: false,
              error: errorMessage,
              userMessage: userFriendlyError,
              details: errorDetails,
            }),
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });
        }
      }

      if (toolResults.length > 0) {
        logger.info('Submitting tool results', {
          resultCount: toolResults.length,
        });
        await submit(toolResults);
      }
    },
    [
      submit,
      executeToolCall,
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

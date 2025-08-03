import { useAssistantContext } from '@/context/AssistantContext';
import { useLocalTools } from '@/context/LocalToolContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/hooks/use-chat';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useCallback, useEffect } from 'react';
import { useAsyncFn } from 'react-use';

interface ToolExecutionResult {
  content: unknown;
  isError?: boolean;
  metadata?: Record<string, unknown>;
}

interface SerializedToolResult {
  success: boolean;
  content?: unknown;
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

  /**
   * Serialize tool execution result with comprehensive error handling
   * Ensures compatibility across all AI service providers (Gemini, Groq, etc.)
   */
  const serializeToolResult = useCallback((
    result: ToolExecutionResult,
    toolName: string,
    executionStartTime: number,
  ): string => {
    const serializedResult: SerializedToolResult = {
      success: !result.isError,
      content: result.isError ? undefined : result.content,
      error: result.isError ? 
        (typeof result.content === 'string' ? result.content : 'Unknown error') : 
        undefined,
      metadata: {
        ...result.metadata,
        toolName,
        isValidated: true,
      },
      toolName,
      executionTime: Date.now() - executionStartTime,
      timestamp: new Date().toISOString(),
    };

    try {
      return JSON.stringify(serializedResult, null, 0);
    } catch (serializationError) {
      // Fallback for non-serializable content
      const fallbackResult: SerializedToolResult = {
        success: false,
        error: `Serialization failed: ${
          serializationError instanceof Error ? serializationError.message : 'Unknown error'
        }`,
        metadata: {
          originalToolName: toolName,
          serializationFailed: true,
        },
        toolName,
        executionTime: Date.now() - executionStartTime,
        timestamp: new Date().toISOString(),
      };
      return JSON.stringify(fallbackResult);
    }
  }, []);

  /**
   * Execute tool calls with comprehensive validation and error handling
   * Follows SynapticFlow guidelines for robust error handling
   */
  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      if (!tcMessage.tool_calls || tcMessage.tool_calls.length === 0) {
        console.warn('No tool calls found in message');
        return;
      }

      const toolResults: Message[] = [];

      for (const toolCall of tcMessage.tool_calls) {
        const toolName = toolCall.function.name;
        const executionStartTime = Date.now();

        try {
          console.info(`Executing tool: ${toolName}`, {
            toolCall: toolCall.function.name,
            args: toolCall.function.arguments,
          });

          const callFunction = isLocalTool(toolName) ? callLocalTool : callMcpTool;
          const result = await callFunction(toolCall);

          // Normalize result to expected interface
          const normalizedResult: ToolExecutionResult = {
            content: result.content,
            isError: ('isError' in result && typeof result.isError === 'boolean') ? result.isError : false,
            metadata: ('metadata' in result && typeof result.metadata === 'object' && result.metadata !== null) 
              ? result.metadata as Record<string, unknown>
              : {},
          };

          const serializedContent = serializeToolResult(
            normalizedResult,
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

          console.info(`Tool executed successfully: ${toolName}`);
        } catch (error) {
          console.error(`Tool execution failed for ${toolName}:`, error);

          const errorResult: ToolExecutionResult = {
            content: error instanceof Error ? error.message : 'Unknown execution error',
            isError: true,
            metadata: {
              errorType: error instanceof Error ? error.constructor.name : 'UnknownError',
              toolCallId: toolCall.id,
            },
          };

          const serializedContent = serializeToolResult(
            errorResult,
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
    [submit, callLocalTool, callMcpTool, schedule, currentAssistant, currentSession, serializeToolResult],
  );

  /**
   * Monitor messages for tool calls and execute them automatically
   * Only executes when assistant message has tool_calls and is not streaming
   */
  useEffect(() => {
    const lastMessage = messages[messages.length - 1];
    if (
      lastMessage &&
      lastMessage.role === 'assistant' &&
      lastMessage.tool_calls &&
      lastMessage.tool_calls.length > 0 &&
      !lastMessage.isStreaming &&
      !loading
    ) {
      execute(lastMessage);
    }
  }, [messages, execute, loading]);

  return null;
};

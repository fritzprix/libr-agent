import { useAssistantContext } from '@/context/AssistantContext';
import { useLocalTools } from '@/context/LocalToolContext';
import { useScheduler } from '@/context/SchedulerContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext } from '@/hooks/use-chat';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useEffect } from 'react';
import { useAsyncFn } from 'react-use';

export const ToolCaller: React.FC = () => {
  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();

  const { messages, submit } = useChatContext();
  const { executeToolCall: callMcpTool } = useMCPServer();
  const { isLocalTool, executeToolCall: callLocalTool } = useLocalTools();
  const { schedule } = useScheduler();
  const [{ loading }, execute] = useAsyncFn(
    async (tcMessage: Message) => {
      const toolResults: Message[] = [];
      for (const toolCall of tcMessage.tool_calls!) {
        const toolName = toolCall.function.name;
        const callFunction = isLocalTool(toolName)
          ? callLocalTool
          : callMcpTool;
        const result = await callFunction(toolCall);
        toolResults.push({
          id: createId(),
          assistantId: currentAssistant?.id,
          role: 'tool',
          content: result.content,
          tool_call_id: toolCall.id,
          sessionId: currentSession?.id || '', // Add sessionId
        });
      }
      schedule(async () => {
        await submit(toolResults);
      });
    },
    [submit, callLocalTool, callMcpTool, schedule],
  );

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
  }, [messages, execute]);

  return null;
};

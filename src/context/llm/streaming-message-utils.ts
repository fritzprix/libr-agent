import type { TokenUsage } from '@/lib/ai-service/types';
import type {
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { Message, ToolCall } from '@/models/chat';

export function extractToolCalls(content: MCPContent[]): ToolCall[] {
  return content
    .filter((item) => item.type === 'tool_call')
    .map((item) => {
      const toolCall = item as MCPToolCallContent;
      return {
        id: toolCall.id,
        type: 'function',
        function: {
          name: toolCall.name,
          arguments: toolCall.arguments,
        },
      };
    });
}

export function extractThinkingText(content: MCPContent[]): string | undefined {
  const thinking = content
    .filter((item) => item.type === 'thinking')
    .map((item) => (item as MCPThinkingContent).thinking)
    .join('\n');

  return thinking || undefined;
}

export function buildStreamingMessage(
  baseMessage: Partial<Message>,
  content: MCPContent[],
  thinkingSignature?: string,
  thinkingTime?: number,
  usage?: TokenUsage,
): Partial<Message> {
  const toolCalls = extractToolCalls(content);

  return {
    ...baseMessage,
    content,
    tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
    thinking: extractThinkingText(content),
    thinkingSignature,
    thinkingTime,
    usage,
    isStreaming: true,
  };
}

export function hasRenderableAssistantOutput(message: Message): boolean {
  return (
    message.content.some((item) =>
      item.type === 'text' ? !!(item as MCPTextContent).text?.trim() : true,
    ) ||
    !!message.tool_calls?.length ||
    !!message.thinking
  );
}

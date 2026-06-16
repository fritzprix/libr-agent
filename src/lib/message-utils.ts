import type { Message, RustMessage } from '@/models/chat';

export {
  messagesToMarkdown,
  type MessagesToMarkdownOptions,
  type MessagesToMarkdownResult,
} from '@/lib/message-markdown';

export function extractTextContent(
  message: Pick<Message, 'content'> | Partial<Message>,
): string {
  return (message.content ?? [])
    .filter(
      (item): item is { type: 'text'; text: string } =>
        item.type === 'text' && typeof item.text === 'string',
    )
    .map((item) => item.text)
    .join('');
}

export function toRustMessage(message: Message): RustMessage {
  const now = Date.now();
  return {
    ...message,
    toolCalls: message.tool_calls,
    toolCallId: message.tool_call_id,
    createdAt:
      message.createdAt instanceof Date
        ? message.createdAt.getTime()
        : message.createdAt || now,
    updatedAt:
      message.updatedAt instanceof Date
        ? message.updatedAt.getTime()
        : message.updatedAt ||
          (message.createdAt instanceof Date
            ? message.createdAt.getTime()
            : message.createdAt) ||
          now,
  };
}

export function summarizeMessageForLog(
  message: Message | Partial<Message> | undefined,
) {
  if (!message) {
    return null;
  }

  return {
    id: message.id,
    role: message.role,
    isStreaming: message.isStreaming,
    contentTypes: Array.isArray(message.content)
      ? message.content.map((item) => item.type)
      : [],
    textLength: extractTextContent(message).length,
    thinkingLength: message.thinking?.length ?? 0,
    toolCallCount: message.tool_calls?.length ?? 0,
    toolCalls: (message.tool_calls ?? []).map((toolCall) => ({
      id: toolCall.id,
      name: toolCall.function.name,
      argumentsLength: toolCall.function.arguments.length,
    })),
  };
}

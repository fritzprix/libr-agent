import type { Message, RustMessage } from '@/models/chat';

function toTimestamp(
  value: Message['createdAt'] | Message['updatedAt'] | undefined,
): number | null {
  if (!value) return null;
  if (value instanceof Date) return value.getTime();
  return typeof value === 'number' ? value : null;
}

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

function persistedToolCallsCoverStreamingState(
  streamingMessage: Message,
  persistedMessage: Message,
): boolean {
  const streamingToolCalls = streamingMessage.tool_calls ?? [];
  if (streamingToolCalls.length === 0) {
    return true;
  }

  const persistedToolCalls = persistedMessage.tool_calls ?? [];
  if (persistedToolCalls.length < streamingToolCalls.length) {
    return false;
  }

  return streamingToolCalls.every((streamingToolCall, index) => {
    const persistedToolCall = persistedToolCalls[index];
    if (!persistedToolCall) {
      return false;
    }

    if (
      streamingToolCall.id &&
      persistedToolCall.id &&
      streamingToolCall.id !== persistedToolCall.id
    ) {
      return false;
    }

    if (
      streamingToolCall.function.name &&
      persistedToolCall.function.name !== streamingToolCall.function.name
    ) {
      return false;
    }

    const streamingArguments = streamingToolCall.function.arguments || '';
    const persistedArguments = persistedToolCall.function.arguments || '';

    return (
      persistedArguments.length >= streamingArguments.length &&
      persistedArguments.startsWith(streamingArguments)
    );
  });
}

export function isAssistantStreamingMessageSuperseded(
  streamingMessage: Message,
  persistedMessage: Message,
): boolean {
  if (
    streamingMessage.role !== 'assistant' ||
    persistedMessage.role !== 'assistant'
  ) {
    return false;
  }

  const streamingTimestamp =
    toTimestamp(streamingMessage.updatedAt) ??
    toTimestamp(streamingMessage.createdAt);
  const persistedTimestamp =
    toTimestamp(persistedMessage.updatedAt) ??
    toTimestamp(persistedMessage.createdAt);

  if (
    streamingTimestamp === null ||
    persistedTimestamp === null ||
    persistedTimestamp < streamingTimestamp
  ) {
    return false;
  }

  const streamingThinking = streamingMessage.thinking || '';
  const persistedThinking = persistedMessage.thinking || '';
  if (
    streamingThinking &&
    (persistedThinking.length < streamingThinking.length ||
      !persistedThinking.startsWith(streamingThinking))
  ) {
    return false;
  }

  const streamingText = extractTextContent(streamingMessage);
  const persistedText = extractTextContent(persistedMessage);
  if (
    streamingText &&
    (persistedText.length < streamingText.length ||
      !persistedText.startsWith(streamingText))
  ) {
    return false;
  }

  return persistedToolCallsCoverStreamingState(
    streamingMessage,
    persistedMessage,
  );
}

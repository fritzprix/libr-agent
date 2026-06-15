import type { Message, ToolCall } from '@/models/chat';
import { extractTextContent } from '@/lib/message-utils';

function toTimestamp(
  value: Message['createdAt'] | Message['updatedAt'] | undefined,
): number | null {
  if (!value) return null;
  if (value instanceof Date) return value.getTime();
  return typeof value === 'number' ? value : null;
}

function resolvePersistedToolCall(
  streamingToolCall: ToolCall,
  persistedToolCalls: ToolCall[],
  index: number,
): ToolCall | undefined {
  const atIndex = persistedToolCalls[index];
  if (!streamingToolCall.id) {
    return atIndex;
  }

  const byId = persistedToolCalls.find(
    (toolCall) => toolCall.id === streamingToolCall.id,
  );
  return byId ?? atIndex;
}

/**
 * Returns whether a persisted assistant message's tool calls cover the
 * in-flight streaming tool-call snapshot.
 *
 * Matching rules per streaming tool call:
 * - Resolve the persisted counterpart by shared `id` when present, otherwise by index.
 * - Reject when names differ.
 * - When both sides share the same non-empty `id`, treat as covered (handles retries
 *   that replace arguments while reusing the call id).
 * - Otherwise require persisted arguments to extend the streaming prefix (incremental JSON).
 */
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
    const persistedToolCall = resolvePersistedToolCall(
      streamingToolCall,
      persistedToolCalls,
      index,
    );
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

    if (
      streamingToolCall.id &&
      persistedToolCall.id &&
      streamingToolCall.id === persistedToolCall.id
    ) {
      return true;
    }

    const streamingArguments = streamingToolCall.function.arguments || '';
    const persistedArguments = persistedToolCall.function.arguments || '';

    return (
      persistedArguments.length >= streamingArguments.length &&
      persistedArguments.startsWith(streamingArguments)
    );
  });
}

/**
 * Determines whether a persisted assistant message supersedes a streaming
 * assistant placeholder so the UI can drop the streaming copy.
 *
 * All of the following must pass (logical AND):
 * - Both messages are assistant role.
 * - Persisted `updatedAt`/`createdAt` is at least as new as streaming.
 * - Thinking text in persisted extends the streaming prefix (when streaming had thinking).
 * - Visible text content in persisted extends the streaming prefix (when streaming had text).
 * - Tool calls in persisted cover the streaming snapshot (see
 *   `persistedToolCallsCoverStreamingState`).
 */
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

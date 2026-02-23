import type { Message, ToolCall } from '@/models/chat';

export const createMessage = (
  id: string,
  role: 'user' | 'assistant' | 'tool',
  content: string = '',
  toolCalls?: ToolCall[],
  toolCallId?: string,
  metadata?: Message['metadata'],
): Message => ({
  id,
  sessionId: 'session-1',
  threadId: 'session-1',
  role,
  content: [{ type: 'text', text: content }],
  tool_calls: toolCalls,
  tool_call_id: toolCallId,
  metadata,
});

import { renderHook } from '@testing-library/react';
import { useMessageGrouping } from '../useMessageGrouping';
import type { Message, ToolCall } from '@/models/chat';
import { describe, it, expect } from 'vitest';

const createMessage = (
  id: string,
  role: 'user' | 'assistant' | 'tool',
  content: string = '',
  toolCalls?: ToolCall[],
  toolCallId?: string,
  metadata?: Message['metadata']
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

describe('useMessageGrouping - Performance Optimization Logic Check', () => {
  it('merges consecutive assistant messages if the second one has ONLY whitespace content', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      // This message has whitespace content. It SHOULD be merged into the previous group.
      // (Because hasTextContent returns false for whitespace-only strings)
      createMessage('3', 'assistant', '   \n   ', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Should result in:
    // 1. User (single)
    // 2. Assistant group (Msg 2 + Msg 3 merged)
    expect(result.current.groupedMessages).toHaveLength(2);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    if (result.current.groupedMessages[1].type === 'tool_group') {
      expect(result.current.groupedMessages[1].toolGroup.calls).toHaveLength(2);
      expect(result.current.groupedMessages[1].messages).toHaveLength(2);
    }
  });

  it('does NOT merge consecutive assistant messages if the second one has non-whitespace content', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      // This message has actual text content. It SHOULD start a NEW group.
      createMessage('3', 'assistant', '   But wait...   ', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Should result in:
    // 1. User (single)
    // 2. Assistant group (Msg 2)
    // 3. Assistant group (Msg 3)
    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
  });

  it('merges assistant message with whitespace-only thinking content', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      {
        ...createMessage('3', 'assistant', '', [
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool2', arguments: '{}' },
          },
        ]),
        thinking: '   \n   ', // Whitespace thinking
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Should merge because thinking is whitespace only -> hasTextContent = false
    expect(result.current.groupedMessages).toHaveLength(2);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    if (result.current.groupedMessages[1].type === 'tool_group') {
      expect(result.current.groupedMessages[1].messages).toHaveLength(2);
    }
  });

  it('does NOT merge assistant message with actual thinking content', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      {
        ...createMessage('3', 'assistant', '', [
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool2', arguments: '{}' },
          },
        ]),
        thinking: ' I am thinking... ', // Actual thinking content
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Should NOT merge because thinking has content -> hasTextContent = true
    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
  });
});

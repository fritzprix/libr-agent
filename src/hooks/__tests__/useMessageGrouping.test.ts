import { renderHook } from '@testing-library/react';
import { useMessageGrouping } from '../useMessageGrouping';
import type { Message, ToolCall } from '@/models/chat';
import { describe, it, expect } from 'vitest';

const createMessage = (
  id: string,
  role: 'user' | 'assistant' | 'tool',
  content: string = '',
  toolCalls?: ToolCall[],
  toolCallId?: string
): Message => ({
  id,
  sessionId: 'session-1',
  threadId: 'session-1',
  role,
  content: [{ type: 'text', text: content }],
  tool_calls: toolCalls,
  tool_call_id: toolCallId,
});

describe('useMessageGrouping', () => {
  it('groups assistant messages with tool calls and populates toolMap', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', 'Calling tool...', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'test_tool', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(2);
    expect(result.current.groupedMessages[0].type).toBe('single');
    expect(result.current.groupedMessages[1].type).toBe('tool_group');

    const group = result.current.groupedMessages[1];
    if (group.type === 'tool_group') {
      expect(group.toolGroup.calls).toHaveLength(1);
      expect(group.toolGroup.calls[0].id).toBe('call_1');
      // Verify pre-calculated results
      expect(group.toolGroup.results).toHaveLength(1);
      expect(group.toolGroup.results[0]).toBeDefined();
      expect(group.toolGroup.results[0]?.id).toBe('3');
    }

    // Verify toolMap
    expect(result.current.toolResultsMap.size).toBe(1);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_1')?.id).toBe('3');
  });

  it('skips standalone tool results but captures them in map', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', 'Calling tool...', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'test_tool', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('4', 'tool', 'Result 2 (Orphan)', undefined, 'call_orphan'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // The orphan tool result is skipped by "if (msg.role === 'tool') continue"
    // The grouped tool result is skipped by the inner loop
    expect(result.current.groupedMessages).toHaveLength(2);

    // Verify toolMap captures BOTH
    expect(result.current.toolResultsMap.size).toBe(2);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_orphan')).toBeDefined();
  });

  it('groups multiple tool calls from consecutive assistant messages and captures all results', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      createMessage('3', 'assistant', '', [
         {
           id: 'call_2',
           type: 'function',
           function: { name: 'tool2', arguments: '{}' },
         },
       ]),
      createMessage('4', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('5', 'tool', 'Result 2', undefined, 'call_2'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(2);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');

    const group = result.current.groupedMessages[1];
    if (group.type === 'tool_group') {
      expect(group.toolGroup.calls).toHaveLength(2);
      expect(group.toolGroup.calls[0].id).toBe('call_1');
      expect(group.toolGroup.calls[1].id).toBe('call_2');
      // Verify pre-calculated results
      expect(group.toolGroup.results).toHaveLength(2);
      expect(group.toolGroup.results[0]?.id).toBe('4');
      expect(group.toolGroup.results[1]?.id).toBe('5');
    }

    expect(result.current.toolResultsMap.size).toBe(2);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_2')).toBeDefined();
  });
});

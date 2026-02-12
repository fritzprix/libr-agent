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

  it('does NOT group consecutive assistant messages if the second one has thinking content', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', '', [
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
        thinking: 'I need to run tool 2 now.',
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Expected behavior after fix:
    // Group 1: Single (User)
    // Group 2: Tool Group (Msg 2 + call_1)
    // Group 3: Single (Msg 3 + thinking + call_2) - NOT merged because of thinking

    // CURRENT BROKEN BEHAVIOR:
    // It groups them, effectively hiding the thinking content of Msg 3 because 
    // the group only keeps the "main" message (Msg 2) and the list of tool calls.
    // So we expect this test to FAIL if we assert they are separate.

    // For reproduction, we assert the DESIRED behavior.
    // Msg 3 has tool calls, so it should be a tool_group, but it should be SEPARATE from Group 2.
    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].message.id).toBe('3');
  });
  it('preserves all messages in a tool group', () => {
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
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(2);
    const group = result.current.groupedMessages[1];

    expect(group.type).toBe('tool_group');
    if (group.type === 'tool_group') {
      expect(group.messages).toHaveLength(2);
      expect(group.messages[0].id).toBe('2');
      expect(group.messages[1].id).toBe('3');
    }
  });

  it('starts a new tool_error_group when a tool result is marked as toolError', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      // A normal tool result is consumed by the tool_group (assistant tool_calls + results)
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      // A failed tool result that is NOT immediately after its triggering assistant tool_call.
      // This represents an orphan/standalone tool failure that should start a tool_error_group.
      createMessage('4', 'tool', 'Error: bad args', undefined, 'call_orphan', {
        toolError: true,
      }),
      // Next assistant message should not be consumed by tool_error_group
      createMessage('5', 'assistant', 'I will try again'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

  // Expected:
  // 1) user single
  // 2) tool_group (assistant tool_calls + tool result)
  // 3) tool_error_group (orphan failed tool result)
  // 4) assistant single
  expect(result.current.groupedMessages).toHaveLength(4);
    expect(result.current.groupedMessages[0].type).toBe('single');
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_error_group');
    expect(result.current.groupedMessages[3].type).toBe('single');

    const errorGroup = result.current.groupedMessages[2];
    if (errorGroup.type === 'tool_error_group') {
      expect(errorGroup.messages).toHaveLength(1);
      expect(errorGroup.messages[0].id).toBe('4');
    }
  });

  it('groups consecutive toolError tool results into a single tool_error_group', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      // Normal tool result consumed by the tool_group
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      // Two consecutive orphan tool failures should form ONE tool_error_group
      createMessage('4', 'tool', 'Error: bad args', undefined, 'call_orphan', {
        toolError: true,
      }),
      createMessage('5', 'tool', 'Error: still bad args', undefined, 'call_orphan', {
        toolError: true,
      }),
      createMessage('6', 'assistant', 'Ok, changing approach'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

  expect(result.current.groupedMessages).toHaveLength(4);
    expect(result.current.groupedMessages[2].type).toBe('tool_error_group');

    const errorGroup = result.current.groupedMessages[2];
    if (errorGroup.type === 'tool_error_group') {
      expect(errorGroup.messages).toHaveLength(2);
      expect(errorGroup.messages[0].id).toBe('4');
      expect(errorGroup.messages[1].id).toBe('5');
    }
  });

  it('maintains referential stability for unchanged prefix', () => {
    const msg1 = createMessage('1', 'user', 'Hello');
    const msg2 = createMessage('2', 'assistant', 'Hi');
    const msg3 = createMessage('3', 'user', 'Bye');

    const messages1 = [msg1, msg2];
    const { result, rerender } = renderHook(({ msgs }) => useMessageGrouping(msgs), {
      initialProps: { msgs: messages1 },
    });

    const firstResult = result.current;

    // Add a new message
    const messages2 = [msg1, msg2, msg3];
    rerender({ msgs: messages2 });

    const secondResult = result.current;

    // First group should be strictly equal (same object reference)
    expect(secondResult.groupedMessages[0]).toBe(firstResult.groupedMessages[0]);

    // OPTIMIZATION UPDATE:
    // The second group (msg2) ends exactly at divergence index. Previously, it was re-evaluated.
    // However, since it is a 'single' message type, it cannot consume subsequent messages.
    // Thus, it is safe to reuse it, ensuring referential stability for the stable prefix.
    expect(secondResult.groupedMessages[1]).toBe(firstResult.groupedMessages[1]);

    // But content should be same
    expect(secondResult.groupedMessages[1].message.id).toBe(firstResult.groupedMessages[1].message.id);

    // The toolResultsMap is now stable if content hasn't changed (empty in this case)
    // optimization: reuses previous map instance
    expect(secondResult.toolResultsMap).toBe(firstResult.toolResultsMap);
    expect(secondResult.toolResultsMap.size).toBe(firstResult.toolResultsMap.size);
    expect(secondResult.toolResultsMap.size).toBe(0);

    // The third group is new
    expect(secondResult.groupedMessages).toHaveLength(3);
    expect(secondResult.groupedMessages[2].message.id).toBe('3');
  });

  it('correctly merges a new tool result into an existing assistant group', () => {
    const msgAssistant = createMessage('1', 'assistant', 'Calling tool...', [
      {
        id: 'call_1',
        type: 'function',
        function: { name: 'test_tool', arguments: '{}' },
      },
    ]);

    // Step 1: Just the assistant message
    const messages1 = [msgAssistant];
    const { result, rerender } = renderHook(({ msgs }) => useMessageGrouping(msgs), {
      initialProps: { msgs: messages1 },
    });

    expect(result.current.groupedMessages).toHaveLength(1);
    expect(result.current.groupedMessages[0].type).toBe('tool_group');
    if (result.current.groupedMessages[0].type === 'tool_group') {
      expect(result.current.groupedMessages[0].toolGroup.results).toHaveLength(1);
      expect(result.current.groupedMessages[0].toolGroup.results[0]).toBeUndefined();
    }

    // Step 2: Add the tool result
    const msgTool = createMessage('2', 'tool', 'Result 1', undefined, 'call_1');
    const messages2 = [msgAssistant, msgTool];
    rerender({ msgs: messages2 });

    expect(result.current.groupedMessages).toHaveLength(1); // Should still be 1 group!
    expect(result.current.groupedMessages[0].type).toBe('tool_group');

    if (result.current.groupedMessages[0].type === 'tool_group') {
       // The result should now be present
       expect(result.current.groupedMessages[0].toolGroup.results[0]).toBeDefined();
       expect(result.current.groupedMessages[0].toolGroup.results[0]?.id).toBe('2');
    }
  });

  it('does not contain stale toolResultsMap entries when messages are removed', () => {
    // Step 1: Start with messages including tool calls and their results
    const messages1: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', 'Calling tool...', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'test_tool', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('4', 'user', 'Another request'),
      createMessage('5', 'assistant', 'Calling another tool...', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'another_tool', arguments: '{}' },
        },
      ]),
      createMessage('6', 'tool', 'Result 2', undefined, 'call_2'),
    ];

    const { result, rerender } = renderHook(({ msgs }) => useMessageGrouping(msgs), {
      initialProps: { msgs: messages1 },
    });

    // Verify initial state has both tool results in map
    expect(result.current.toolResultsMap.size).toBe(2);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_2')).toBeDefined();

    // Step 2: Remove messages from the end (last 3 messages)
    const messages2 = messages1.slice(0, 3); // Keep only first 3 messages
    rerender({ msgs: messages2 });

    // Verify toolResultsMap does NOT contain stale entry for 'call_2'
    expect(result.current.toolResultsMap.size).toBe(1);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_2')).toBeUndefined();

    // Verify groupedMessages correctly reflects remaining messages
    expect(result.current.groupedMessages).toHaveLength(2); // user message + tool_group

    // Step 3: Remove more messages (remove tool call and result)
    const messages3 = messages1.slice(0, 1); // Keep only first message
    rerender({ msgs: messages3 });

    // Verify toolResultsMap is empty (no tool results exist anymore)
    expect(result.current.toolResultsMap.size).toBe(0);
    expect(result.current.toolResultsMap.get('call_1')).toBeUndefined();

    // Verify only user message remains
    expect(result.current.groupedMessages).toHaveLength(1);
    expect(result.current.groupedMessages[0].type).toBe('single');
    expect(result.current.groupedMessages[0].message.id).toBe('1');
  });
});

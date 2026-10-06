import { renderHook } from '@testing-library/react';
import { useMessageGrouping } from '../useMessageGrouping';
import type { Message, MessageSource } from '@/models/chat';
import { describe, it, expect } from 'vitest';
import { StrictMode, createElement } from 'react';
import { createMessage } from './helpers';

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
      expect(group.coveredMessageIds).toEqual(['2', '3']);
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

  it('tracks grouped tool result ids in coveredMessageIds', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));
    const group = result.current.groupedMessages[1];

    expect(group.type).toBe('tool_group');
    if (group.type === 'tool_group') {
      expect(group.messages.map((message) => message.id)).toEqual(['2']);
      expect(group.coveredMessageIds).toEqual(['2', '3']);
    }
  });

  it('keeps each assistant tool response as its own tool_group (response unit)', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('4', 'assistant', '', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
      createMessage('5', 'tool', 'Result 2', undefined, 'call_2'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');

    const group1 = result.current.groupedMessages[1];
    const group2 = result.current.groupedMessages[2];
    if (group1.type === 'tool_group' && group2.type === 'tool_group') {
      expect(group1.toolGroup.calls).toHaveLength(1);
      expect(group1.toolGroup.calls[0].id).toBe('call_1');
      expect(group1.toolGroup.results[0]?.id).toBe('3');
      expect(group1.messages).toHaveLength(1);

      expect(group2.toolGroup.calls).toHaveLength(1);
      expect(group2.toolGroup.calls[0].id).toBe('call_2');
      expect(group2.toolGroup.results[0]?.id).toBe('5');
      expect(group2.messages).toHaveLength(1);
    }

    expect(result.current.toolResultsMap.size).toBe(2);
    expect(result.current.toolResultsMap.get('call_1')).toBeDefined();
    expect(result.current.toolResultsMap.get('call_2')).toBeDefined();
  });

  it('still groups parallel tool calls from a single assistant response', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('4', 'tool', 'Result 2', undefined, 'call_2'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(2);
    const group = result.current.groupedMessages[1];
    expect(group.type).toBe('tool_group');
    if (group.type === 'tool_group') {
      expect(group.toolGroup.calls).toHaveLength(2);
      expect(group.toolGroup.results[0]?.id).toBe('3');
      expect(group.toolGroup.results[1]?.id).toBe('4');
    }
  });

  it('does NOT merge consecutive assistant messages even without thinking content', () => {
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

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].message.id).toBe('3');
  });

  it('preserves only the assistant message for each response-unit tool group', () => {
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

    expect(result.current.groupedMessages).toHaveLength(3);
    const group1 = result.current.groupedMessages[1];
    const group2 = result.current.groupedMessages[2];

    expect(group1.type).toBe('tool_group');
    expect(group2.type).toBe('tool_group');
    if (group1.type === 'tool_group' && group2.type === 'tool_group') {
      expect(group1.messages).toHaveLength(1);
      expect(group1.messages[0].id).toBe('2');
      expect(group2.messages).toHaveLength(1);
      expect(group2.messages[0].id).toBe('3');
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

  it('handles duplicate tool_call_ids generated by LLM hallucinations (appends _dup suffix)', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tool multiple times'),
      createMessage('2', 'assistant', 'Calling tool...', [
        {
          id: 'hallucinated_id',
          type: 'function',
          function: { name: 'test_tool', arguments: '{"arg": 1}' },
        },
        {
          id: 'hallucinated_id', // Duplicate ID
          type: 'function',
          function: { name: 'test_tool', arguments: '{"arg": 2}' },
        },
      ]),
      createMessage('3', 'tool', 'Result 1', undefined, 'hallucinated_id'),
      createMessage('4', 'tool', 'Result 2', undefined, 'hallucinated_id'), // Result for duplicate
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(2);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');

    const group = result.current.groupedMessages[1];
    if (group.type === 'tool_group') {
      expect(group.toolGroup.calls).toHaveLength(2);
      expect(group.toolGroup.calls[0].id).toBe('hallucinated_id');
      expect(group.toolGroup.calls[1].id).toBe('hallucinated_id');
      
      // Verify pre-calculated results are correctly mapped without overwriting
      expect(group.toolGroup.results).toHaveLength(2);
      expect(group.toolGroup.results[0]?.id).toBe('3');
      expect(group.toolGroup.results[1]?.id).toBe('4');
    }

    // Verify toolMap correctly isolated them
    expect(result.current.toolResultsMap.size).toBe(2);
    expect(result.current.toolResultsMap.get('hallucinated_id')?.id).toBe('3');
    expect(result.current.toolResultsMap.get('hallucinated_id_dup1')?.id).toBe('4');
  });

  it('keeps committed grouping stable across StrictMode rerenders', () => {
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

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      createElement(StrictMode, null, children)
    );

    const { result, rerender } = renderHook(
      ({ msgs }) => useMessageGrouping(msgs),
      {
        initialProps: { msgs: messages },
        wrapper,
      },
    );

    const first = result.current;
    rerender({ msgs: messages });

    expect(result.current.groupedMessages).toEqual(first.groupedMessages);
    expect(result.current.toolResultsMap).toBe(first.toolResultsMap);
  });

  it('filters out internal scaffolding messages (session-context, compaction-instruction)', () => {
    const messages: Message[] = [
      { ...createMessage('1', 'user', 'Normal message'), source: 'ui' as MessageSource },
      { ...createMessage('2', 'user', 'Session context background info'), source: 'session-context' as MessageSource },
      { ...createMessage('3', 'user', 'Compaction request'), source: 'compaction-instruction' as MessageSource },
      {
        ...createMessage(
          '4',
          'tool',
          'Recovery tombstone',
          undefined,
          'tool_call_1',
          { toolError: true },
        ),
        source: 'recovery' as MessageSource,
      },
      { ...createMessage('5', 'assistant', 'Response message'), source: 'api' as MessageSource },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    // Recovery tombstones stay visible so orphaned tool calls can resolve.
    // Standalone toolError results render as tool_error_group.
    expect(result.current.groupedMessages.length).toBe(3);
    expect(result.current.groupedMessages[0].message.id).toBe('1');
    expect(result.current.groupedMessages[1].type).toBe('tool_error_group');
    expect(result.current.groupedMessages[1].message.id).toBe('4');
    expect(result.current.groupedMessages[2].message.id).toBe('5');
  });

  // Paired with session_recovery_tests.rs: backend tombstones use source=recovery + toolError.
  // Filtering source=recovery here recreates stuck tool-call spinners after crash recovery.
  it('matches recovery tombstones to orphaned tool calls (no stuck spinner)', () => {
    const messages: Message[] = [
      createMessage('1', 'assistant', '', [
        {
          id: 'call_orphan',
          type: 'function',
          function: { name: 'agent__messageToSession', arguments: '{}' },
        },
      ]),
      {
        ...createMessage(
          '2',
          'tool',
          '[system] Tool call did not complete (session recovered after crash).',
          undefined,
          'call_orphan',
          { toolError: true },
        ),
        source: 'recovery' as MessageSource,
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(1);
    const group = result.current.groupedMessages[0];
    expect(group.type).toBe('tool_group');
    if (group.type === 'tool_group') {
      expect(group.toolGroup.results).toHaveLength(1);
      expect(group.toolGroup.results[0]).toBeDefined();
      expect(group.toolGroup.results[0]?.id).toBe('2');
      expect(group.toolGroup.results[0]?.metadata?.toolError).toBe(true);
    }
    expect(result.current.toolResultsMap.get('call_orphan')?.id).toBe('2');
  });

  it('splits tool_group into separate groups at the specified boundaryId (compaction boundary)', () => {
    const messages: Message[] = [
      {
        ...createMessage('1', 'assistant', ''),
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'tool_1', arguments: '{}' },
          },
        ],
      },
      {
        ...createMessage('2', 'tool', 'Result 1'),
        tool_call_id: 'call_1',
      },
      {
        ...createMessage('3', 'assistant', ''),
        tool_calls: [
          {
            id: 'call_2',
            type: 'function',
            function: { name: 'tool_2', arguments: '{}' },
          },
        ],
      },
      {
        ...createMessage('4', 'tool', 'Result 2'),
        tool_call_id: 'call_2',
      },
    ];

    // Without boundaryId, each assistant response is already its own tool_group
    const { result: withoutBoundary } = renderHook(() =>
      useMessageGrouping(messages),
    );
    expect(withoutBoundary.current.groupedMessages.length).toBe(2);

    // With boundaryId set to '2' (tool result 1), response units stay intact;
    // divider placement uses coveredMessageIds, not truncated consumption.
    const { result: withBoundary } = renderHook(() =>
      useMessageGrouping(messages, '2'),
    );
    expect(withBoundary.current.groupedMessages.length).toBe(2);
    const group0 = withBoundary.current.groupedMessages[0];
    const group1 = withBoundary.current.groupedMessages[1];
    expect(group0.type).toBe('tool_group');
    if (group0.type === 'tool_group') {
      expect(group0.coveredMessageIds).toEqual(['1', '2']);
    }
    expect(group1.type).toBe('tool_group');
    if (group1.type === 'tool_group') {
      expect(group1.coveredMessageIds).toEqual(['3', '4']);
    }
  });

  it('keeps parallel tool results when boundaryId matches the first result (#audit)', () => {
    const messages: Message[] = [
      createMessage('1', 'assistant', '', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool_1', arguments: '{}' },
        },
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool_2', arguments: '{}' },
        },
      ]),
      createMessage('2', 'tool', 'Result 1', undefined, 'call_1'),
      createMessage('3', 'tool', 'Result 2', undefined, 'call_2'),
      createMessage('4', 'assistant', '', [
        {
          id: 'call_3',
          type: 'function',
          function: { name: 'tool_3', arguments: '{}' },
        },
      ]),
      createMessage('5', 'tool', 'Result 3', undefined, 'call_3'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages, '2'));

    expect(result.current.groupedMessages).toHaveLength(2);
    const group0 = result.current.groupedMessages[0];
    expect(group0.type).toBe('tool_group');
    if (group0.type === 'tool_group') {
      expect(group0.coveredMessageIds).toEqual(['1', '2', '3']);
      expect(group0.toolGroup.results).toHaveLength(2);
      expect(group0.toolGroup.results[0]?.id).toBe('2');
      expect(group0.toolGroup.results[1]?.id).toBe('3');
    }
  });

  it('maps each response unit to its own result when tool_call_id is reused (#audit)', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', '', [
        {
          id: 'reused_id',
          type: 'function',
          function: { name: 'tool', arguments: '{"n":1}' },
        },
      ]),
      createMessage('3', 'tool', 'Result turn 1', undefined, 'reused_id'),
      createMessage('4', 'assistant', '', [
        {
          id: 'reused_id',
          type: 'function',
          function: { name: 'tool', arguments: '{"n":2}' },
        },
      ]),
      createMessage('5', 'tool', 'Result turn 2', undefined, 'reused_id'),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    const group1 = result.current.groupedMessages[1];
    const group2 = result.current.groupedMessages[2];
    expect(group1.type).toBe('tool_group');
    expect(group2.type).toBe('tool_group');
    if (group1.type === 'tool_group' && group2.type === 'tool_group') {
      expect(group1.toolGroup.results[0]?.id).toBe('3');
      expect(group2.toolGroup.results[0]?.id).toBe('5');
    }

    // Session map still isolates duplicates for other consumers.
    expect(result.current.toolResultsMap.get('reused_id')?.id).toBe('3');
    expect(result.current.toolResultsMap.get('reused_id_dup1')?.id).toBe('5');
  });
});

import { renderHook } from '@testing-library/react';
import { useMessageGrouping } from '../useMessageGrouping';
import type { Message } from '@/models/chat';
import { describe, it, expect } from 'vitest';
import { createMessage } from './helpers';

describe('useMessageGrouping - response-unit boundaries', () => {
  it('does not merge consecutive assistant tool responses even with whitespace-only text', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      createMessage('3', 'assistant', '   \n   ', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
    if (
      result.current.groupedMessages[1].type === 'tool_group' &&
      result.current.groupedMessages[2].type === 'tool_group'
    ) {
      expect(result.current.groupedMessages[1].toolGroup.calls).toHaveLength(1);
      expect(result.current.groupedMessages[2].toolGroup.calls).toHaveLength(1);
      expect(result.current.groupedMessages[1].messages).toHaveLength(1);
      expect(result.current.groupedMessages[2].messages).toHaveLength(1);
    }
  });

  it('does not merge consecutive assistant tool responses with non-whitespace text', () => {
    const messages: Message[] = [
      createMessage('1', 'user', 'Run tools'),
      createMessage('2', 'assistant', 'Call tool 1', [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'tool1', arguments: '{}' },
        },
      ]),
      createMessage('3', 'assistant', '   But wait...   ', [
        {
          id: 'call_2',
          type: 'function',
          function: { name: 'tool2', arguments: '{}' },
        },
      ]),
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
  });

  it('does not merge consecutive assistant tool responses with whitespace-only thinking', () => {
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
        thinking: '   \n   ',
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
    if (result.current.groupedMessages[2].type === 'tool_group') {
      expect(result.current.groupedMessages[2].messages).toHaveLength(1);
    }
  });

  it('does not merge consecutive assistant tool responses with actual thinking content', () => {
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
        thinking: ' I am thinking... ',
      },
    ];

    const { result } = renderHook(() => useMessageGrouping(messages));

    expect(result.current.groupedMessages).toHaveLength(3);
    expect(result.current.groupedMessages[1].type).toBe('tool_group');
    expect(result.current.groupedMessages[2].type).toBe('tool_group');
  });
});

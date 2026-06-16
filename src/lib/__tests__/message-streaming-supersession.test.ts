import { describe, expect, it } from 'vitest';
import { isAssistantStreamingMessageSuperseded } from '../message-streaming-supersession';
import type { Message } from '@/models/chat';

function assistantMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'assistant-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'assistant',
    content: [],
    createdAt: new Date('2026-04-04T05:00:00.000Z'),
    updatedAt: new Date('2026-04-04T05:00:01.000Z'),
    ...overrides,
  };
}

describe('isAssistantStreamingMessageSuperseded', () => {
  it('requires persisted tool calls to catch up before superseding', () => {
    const streaming = assistantMessage({
      content: [{ type: 'text', text: 'Building artifact...' }],
      thinking: 'Need a tool...',
      tool_calls: [
        {
          id: 'call-streaming',
          type: 'function',
          function: {
            name: 'workspace__writeFile',
            arguments: '{"path":"index.html"',
          },
        },
      ],
    });

    const persistedWithoutTools = assistantMessage({
      id: 'persisted-1',
      content: streaming.content,
      thinking: streaming.thinking,
      tool_calls: [],
      updatedAt: new Date('2026-04-04T05:00:02.000Z'),
    });

    const persistedWithTools = assistantMessage({
      id: 'persisted-2',
      content: streaming.content,
      thinking: streaming.thinking,
      tool_calls: [
        {
          id: 'call-streaming',
          type: 'function',
          function: {
            name: 'workspace__writeFile',
            arguments: '{"path":"index.html","content":"ok"}',
          },
        },
      ],
      updatedAt: new Date('2026-04-04T05:00:03.000Z'),
    });

    expect(
      isAssistantStreamingMessageSuperseded(streaming, persistedWithoutTools),
    ).toBe(false);
    expect(
      isAssistantStreamingMessageSuperseded(streaming, persistedWithTools),
    ).toBe(true);
  });

  it('supersedes when persisted reuses the same tool call id with different arguments', () => {
    const streaming = assistantMessage({
      tool_calls: [
        {
          id: 'call-retry',
          type: 'function',
          function: {
            name: 'workspace__readFile',
            arguments: '{"path":"draft.txt"',
          },
        },
      ],
      updatedAt: new Date('2026-04-04T05:00:01.000Z'),
    });

    const persisted = assistantMessage({
      id: 'persisted-retry',
      tool_calls: [
        {
          id: 'call-retry',
          type: 'function',
          function: {
            name: 'workspace__readFile',
            arguments: '{"path":"final.txt"}',
          },
        },
      ],
      updatedAt: new Date('2026-04-04T05:00:02.000Z'),
    });

    expect(isAssistantStreamingMessageSuperseded(streaming, persisted)).toBe(
      true,
    );
  });

  it('resolves persisted tool calls by id when index order differs', () => {
    const streaming = assistantMessage({
      tool_calls: [
        {
          id: 'call-b',
          type: 'function',
          function: {
            name: 'tool_b',
            arguments: '{"b":1',
          },
        },
      ],
    });

    const persisted = assistantMessage({
      id: 'persisted-order',
      tool_calls: [
        {
          id: 'call-a',
          type: 'function',
          function: { name: 'tool_a', arguments: '{}' },
        },
        {
          id: 'call-b',
          type: 'function',
          function: { name: 'tool_b', arguments: '{"b":123}' },
        },
      ],
      updatedAt: new Date('2026-04-04T05:00:02.000Z'),
    });

    expect(isAssistantStreamingMessageSuperseded(streaming, persisted)).toBe(
      true,
    );
  });

  it('rejects supersession when persisted timestamp is older than streaming', () => {
    const streaming = assistantMessage({
      content: [{ type: 'text', text: 'Hello' }],
      updatedAt: new Date('2026-04-04T05:00:02.000Z'),
    });

    const persisted = assistantMessage({
      id: 'persisted-old',
      content: [{ type: 'text', text: 'Hello world' }],
      updatedAt: new Date('2026-04-04T05:00:01.000Z'),
    });

    expect(isAssistantStreamingMessageSuperseded(streaming, persisted)).toBe(
      false,
    );
  });
});

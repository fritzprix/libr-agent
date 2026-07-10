import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import {
  getLatestMessageScrollFingerprint,
  isThinkingOnlyLatestMessageUpdate,
} from '../utils';

function createMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'assistant',
    content: [],
    isStreaming: true,
    ...overrides,
  };
}

describe('latest message scroll fingerprint', () => {
  it('treats thinking-only updates as layout-neutral', () => {
    const previous = createMessage({
      thinking: 'step one',
      content: [{ type: 'thinking', thinking: 'step one' }],
    });
    const next = createMessage({
      thinking: 'step one\nstep two',
      content: [{ type: 'thinking', thinking: 'step one\nstep two' }],
    });

    expect(getLatestMessageScrollFingerprint(previous)).toBe(
      getLatestMessageScrollFingerprint(next),
    );
    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(true);
  });

  it('detects layout changes when assistant text starts streaming', () => {
    const previous = createMessage({
      thinking: 'step one',
      content: [{ type: 'thinking', thinking: 'step one' }],
    });
    const next = createMessage({
      thinking: 'step one',
      content: [
        { type: 'thinking', thinking: 'step one' },
        { type: 'text', text: 'Hello' },
      ],
    });

    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(false);
  });

  it('detects layout changes when tool calls arrive', () => {
    const previous = createMessage({
      thinking: 'step one',
      content: [{ type: 'thinking', thinking: 'step one' }],
    });
    const next = createMessage({
      thinking: 'step one',
      content: [{ type: 'thinking', thinking: 'step one' }],
      tool_calls: [
        {
          id: 'tool-1',
          type: 'function',
          function: { name: 'search', arguments: '{}' },
        },
      ],
    });

    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(false);
  });
});

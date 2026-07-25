import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import {
  getLatestMessageScrollFingerprint,
  isLayoutNeutralLatestMessageUpdate,
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
    expect(isLayoutNeutralLatestMessageUpdate(previous, next)).toBe(true);
    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(true);
  });

  it('treats Non-Thinking text token growth as layout-neutral', () => {
    const previous = createMessage({
      content: [{ type: 'text', text: 'Hello' }],
    });
    const next = createMessage({
      content: [{ type: 'text', text: 'Hello world, more tokens' }],
    });

    expect(getLatestMessageScrollFingerprint(previous)).toBe(
      getLatestMessageScrollFingerprint(next),
    );
    expect(isLayoutNeutralLatestMessageUpdate(previous, next)).toBe(true);
    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(false);
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

    expect(isLayoutNeutralLatestMessageUpdate(previous, next)).toBe(false);
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

    expect(isLayoutNeutralLatestMessageUpdate(previous, next)).toBe(false);
    expect(isThinkingOnlyLatestMessageUpdate(previous, next)).toBe(false);
  });
});

import { describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';
import {
  applyWorkflowInactiveCleanup,
  isInactiveWorkflowStatus,
  stripMessageStreamingFlags,
} from '../workflow-inactive-cleanup';

function msg(partial: Partial<Message> & Pick<Message, 'id'>): Message {
  return {
    sessionId: 's1',
    threadId: 's1',
    role: 'assistant',
    content: [],
    createdAt: new Date(0),
    ...partial,
  };
}

describe('isInactiveWorkflowStatus', () => {
  it('treats idle/paused/error as inactive', () => {
    expect(isInactiveWorkflowStatus('idle')).toBe(true);
    expect(isInactiveWorkflowStatus('paused')).toBe(true);
    expect(isInactiveWorkflowStatus('error')).toBe(true);
  });

  it('treats busy/queued/provisioning as active', () => {
    expect(isInactiveWorkflowStatus('busy')).toBe(false);
    expect(isInactiveWorkflowStatus('queued')).toBe(false);
    expect(isInactiveWorkflowStatus('provisioning')).toBe(false);
  });
});

describe('stripMessageStreamingFlags', () => {
  it('returns the same array reference when nothing is streaming', () => {
    const messages = [msg({ id: 'a', isStreaming: false })];
    expect(stripMessageStreamingFlags(messages)).toBe(messages);
  });

  it('clears isStreaming without mutating other messages unnecessarily', () => {
    const messages = [
      msg({ id: 'a', isStreaming: true, thinking: '...' }),
      msg({ id: 'b', role: 'user', isStreaming: false }),
    ];
    const next = stripMessageStreamingFlags(messages);
    expect(next).not.toBe(messages);
    expect(next[0].isStreaming).toBe(false);
    expect(next[0].thinking).toBe('...');
    expect(next[1]).toBe(messages[1]);
  });
});

describe('applyWorkflowInactiveCleanup', () => {
  it('clears streaming placeholder and strips message flags without aborting', () => {
    const clearStreamingMessage = vi.fn();
    const setMessages = vi.fn((updater: (prev: Message[]) => Message[]) => {
      const prev = [msg({ id: 'a', isStreaming: true })];
      const next = updater(prev);
      expect(next[0].isStreaming).toBe(false);
    });

    applyWorkflowInactiveCleanup({
      sessionId: 'session-1',
      clearStreamingMessage,
      setMessages,
    });

    expect(clearStreamingMessage).toHaveBeenCalledWith('session-1');
    expect(setMessages).toHaveBeenCalledTimes(1);
  });
});

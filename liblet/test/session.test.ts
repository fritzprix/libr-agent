import { describe, expect, it } from 'vitest';

import { initialState, reduce } from '../src/session.js';
import type { ChatMessage, SessionStatusResponse } from '../src/types.js';

const message = (
  id: string,
  createdAt: number,
  text: string,
  role = 'assistant',
): ChatMessage => ({
  id,
  sessionId: 's1',
  role,
  content: [{ type: 'text', text }],
  createdAt,
  updatedAt: createdAt,
});

const status = (
  over: Partial<SessionStatusResponse> = {},
): SessionStatusResponse => ({
  id: 's1',
  status: 'idle',
  model: 'm',
  provider: 'p',
  createdAt: 0,
  updatedAt: 0,
  executionMode: 'normal',
  pendingApprovals: [],
  ...over,
});

describe('transcript reducer', () => {
  it('history sorts by time', () => {
    const state = reduce(initialState(), {
      type: 'history',
      messages: [message('b', 2, 'second'), message('a', 1, 'first')],
    });
    expect(state.messages.map((m) => m.id)).toEqual(['a', 'b']);
  });

  it('a message event appends new and replaces by id, keeping order', () => {
    let state = reduce(initialState(), {
      type: 'history',
      messages: [message('a', 1, 'one')],
    });
    state = reduce(state, { type: 'message', message: message('b', 2, 'two') });
    state = reduce(state, {
      type: 'message',
      message: message('a', 1, 'one, updated'),
    });
    expect(state.messages.map((m) => m.id)).toEqual(['a', 'b']);
    expect(state.messages[0].content[0].text).toBe('one, updated');
  });

  it('a status snapshot carries approvals, prompt and mode', () => {
    const state = reduce(initialState(), {
      type: 'status',
      status: status({
        status: 'busy',
        executionMode: 'yolo',
        pendingApprovals: [
          {
            toolCallId: 't1',
            toolName: 'shell',
            reason: 'rm',
            commandPreview: 'rm -rf /tmp/x',
            approvalKind: 'standard',
          },
        ],
        pendingUserPrompt: {
          requestId: 'p1',
          kind: 'secret',
          payload: { variableName: 'TOKEN', label: 'token' },
          createdAt: '2026-11-13T00:00:00Z',
        },
      }),
    });
    expect(state.status).toBe('busy');
    expect(state.executionMode).toBe('yolo');
    expect(state.pendingApprovals).toHaveLength(1);
    expect(state.pendingUserPrompt?.requestId).toBe('p1');
  });

  it('end and notice are recorded', () => {
    let state = reduce(initialState(), { type: 'end' });
    expect(state.sessionGone).toBe(true);
    state = reduce(state, { type: 'notice', notice: 'boom' });
    expect(state.notice).toBe('boom');
  });
});

import { beforeEach, describe, expect, it } from 'vitest';

import type { AgentOpenSessionResponse } from '@/models/agent-ipc';
import {
  MAX_CACHED_SESSIONS,
  clearOpenSessionViewCache,
  getOpenSessionView,
  invalidateOpenSessionView,
  isWarmOpenSessionView,
  putOpenSessionView,
} from '../openSessionViewCache';

function makeResponse(
  sessionId: string,
  overrides?: {
    phase?: AgentOpenSessionResponse['runtimeState']['phase'];
    proxy?: Partial<AgentOpenSessionResponse['runtimeState']['proxy']>;
  },
): AgentOpenSessionResponse {
  const timestamp = Date.now();
  return {
    session: {
      id: sessionId,
      name: sessionId,
      status: 'idle',
      model: 'm',
      provider: 'p',
      createdAt: timestamp,
      updatedAt: timestamp,
      executionMode: 'normal',
      workspaceIsolation: 'host',
    },
    messages: {
      items: [],
      hasMoreBefore: false,
      oldestCursor: null,
    },
    pendingApprovals: [],
    runtimeState: {
      sequence: 1,
      phase: overrides?.phase ?? 'ready',
      proxy: {
        exists: true,
        mode: 'builtin_only',
        ready: true,
        ...overrides?.proxy,
      },
      initialization: {
        result: 'success',
      },
      servers: [],
    },
  };
}

describe('openSessionViewCache', () => {
  beforeEach(() => {
    clearOpenSessionViewCache();
  });

  it('stores and returns warm open payloads', () => {
    const response = makeResponse('s1');
    putOpenSessionView('s1', response);

    expect(getOpenSessionView('s1')).toEqual(response);
    expect(isWarmOpenSessionView(response)).toBe(true);
  });

  it('evicts least-recently-used entries past the max size', () => {
    for (let i = 0; i < MAX_CACHED_SESSIONS + 1; i += 1) {
      putOpenSessionView(`s${i}`, makeResponse(`s${i}`));
    }

    expect(getOpenSessionView('s0')).toBeUndefined();
    expect(getOpenSessionView(`s${MAX_CACHED_SESSIONS}`)?.session.id).toBe(
      `s${MAX_CACHED_SESSIONS}`,
    );
  });

  it('treats hydrating/not-ready payloads as cold', () => {
    const cold = makeResponse('s1', {
      phase: 'hydrating',
      proxy: { exists: false, mode: 'none', ready: false },
    });
    expect(isWarmOpenSessionView(cold)).toBe(false);
  });

  it('invalidates a single session', () => {
    putOpenSessionView('s1', makeResponse('s1'));
    invalidateOpenSessionView('s1');
    expect(getOpenSessionView('s1')).toBeUndefined();
  });
});

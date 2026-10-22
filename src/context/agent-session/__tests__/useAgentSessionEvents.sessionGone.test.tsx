import { renderHook, waitFor, act } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Event, EventCallback } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';
import { openAgentSession } from '@/lib/backend/agent-commands';
import type { AgentOpenSessionResponse } from '@/models/agent-ipc';
import type { AgentEventPayload } from '../types';
import { clearOpenSessionViewCache } from '../openSessionViewCache';
import { useAgentSessionEvents } from '../useAgentSessionEvents';
import { useAgentSessionState } from '../useAgentSessionState';

// Real react-router returns a new navigate on every pathname change; a fresh
// function per render reproduces that identity churn.
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  openAgentSession: vi.fn(),
  getAgentSessionMetadata: vi.fn(),
}));

vi.mock('@/lib/backend/assistants', () => ({
  getAssistant: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const listenMock = vi.mocked(listen);
const openAgentSessionMock = vi.mocked(openAgentSession);

const SESSION_ID = 'session-gone-1';

function createOpenResponse(sessionId: string): AgentOpenSessionResponse {
  const timestamp = Date.now();
  return {
    session: {
      id: sessionId,
      name: 'Session',
      status: 'idle',
      model: 'test-model',
      provider: 'test-provider',
      createdAt: timestamp,
      updatedAt: timestamp,
      executionMode: 'normal',
      workspaceIsolation: 'host',
    },
    messages: { items: [], hasMoreBefore: false, oldestCursor: null },
    pendingApprovals: [],
    runtimeState: {
      sequence: 1,
      phase: 'ready',
      proxy: { exists: true, mode: 'builtin_only', ready: true },
      initialization: { currentStep: 'done', result: 'success' },
      servers: [],
    },
  };
}

const persistViewedAt = vi.fn().mockResolvedValue(undefined);
const clearStreamingMessage = vi.fn();

function renderSessionEvents(
  onSessionGone: (sessionId: string, reason: string) => void,
) {
  const actions = { persistViewedAt, clearStreamingMessage, onSessionGone };
  return renderHook(() => {
    const stateProps = useAgentSessionState();
    useAgentSessionEvents(SESSION_ID, stateProps, actions);
    return stateProps.state;
  });
}

describe('useAgentSessionEvents session lifecycle', () => {
  let agentEventHandler: EventCallback<AgentEventPayload> | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    clearOpenSessionViewCache();
    agentEventHandler = undefined;
    listenMock.mockImplementation(async (_event, handler) => {
      agentEventHandler = handler as EventCallback<AgentEventPayload>;
      return () => {};
    });
  });

  it('does not re-open the session when navigate changes identity', async () => {
    openAgentSessionMock.mockResolvedValue(createOpenResponse(SESSION_ID));
    const { rerender } = renderSessionEvents(vi.fn());

    await waitFor(() => {
      expect(openAgentSessionMock).toHaveBeenCalledTimes(1);
    });

    rerender();
    rerender();

    await act(async () => {
      await Promise.resolve();
    });
    expect(openAgentSessionMock).toHaveBeenCalledTimes(1);
  });

  it('reports not_found when open fails with Session not found', async () => {
    openAgentSessionMock.mockRejectedValue(
      new Error(`Session not found: ${SESSION_ID}`),
    );
    const onSessionGone = vi.fn();
    const { result } = renderSessionEvents(onSessionGone);

    await waitFor(() => {
      expect(onSessionGone).toHaveBeenCalledWith(SESSION_ID, 'not_found');
    });
    expect(result.current.session).toBeNull();
    expect(openAgentSessionMock).toHaveBeenCalledTimes(1);
  });

  it('does not report gone for other open failures', async () => {
    openAgentSessionMock.mockRejectedValue(new Error('database is locked'));
    const onSessionGone = vi.fn();
    const { result } = renderSessionEvents(onSessionGone);

    await waitFor(() => {
      expect(result.current.workflowStatus).toBe('error');
    });
    expect(onSessionGone).not.toHaveBeenCalled();
  });

  it('reports deleted on a session delete resourceUpdated event', async () => {
    openAgentSessionMock.mockResolvedValue(createOpenResponse(SESSION_ID));
    const onSessionGone = vi.fn();
    renderSessionEvents(onSessionGone);

    await waitFor(() => {
      expect(agentEventHandler).toBeDefined();
    });

    const deleteEvent: Event<AgentEventPayload> = {
      event: 'agent:event',
      id: 1,
      payload: {
        type: 'resourceUpdated',
        resourceType: 'session',
        action: 'delete',
        resourceId: SESSION_ID,
      },
    };
    act(() => {
      agentEventHandler?.(deleteEvent);
    });

    expect(onSessionGone).toHaveBeenCalledWith(SESSION_ID, 'deleted');
  });
});

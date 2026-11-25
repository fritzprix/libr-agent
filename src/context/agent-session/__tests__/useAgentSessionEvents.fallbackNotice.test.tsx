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

const SESSION_ID = 'session-fb-1';

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

function renderSessionEvents() {
  const actions = { persistViewedAt, clearStreamingMessage, onSessionGone: vi.fn() };
  return renderHook(() => {
    const stateProps = useAgentSessionState();
    useAgentSessionEvents(SESSION_ID, stateProps, actions);
    return stateProps.state;
  });
}

function fallbackNoticeEvent(
  sessionId: string,
  id: number,
): Event<AgentEventPayload> {
  return {
    event: 'agent:event',
    id,
    payload: {
      type: 'llmFallbackNotice',
      sessionId,
      statusCode: 400,
      fallbackProvider: 'openai',
      fallbackModel: 'fallback-model',
      errorMessage: 'LLM provider returned HTTP 400: too large',
    },
  };
}

describe('useAgentSessionEvents llmFallbackNotice (#2961)', () => {
  let agentEventHandler: EventCallback<AgentEventPayload> | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    clearOpenSessionViewCache();
    agentEventHandler = undefined;
    listenMock.mockImplementation(async (_event, handler) => {
      agentEventHandler = handler as EventCallback<AgentEventPayload>;
      return () => {};
    });
    openAgentSessionMock.mockResolvedValue(createOpenResponse(SESSION_ID));
  });

  it('appends a system notice message with metadata.fallbackNotice', async () => {
    const { result } = renderSessionEvents();
    await waitFor(() => {
      expect(agentEventHandler).toBeDefined();
    });

    act(() => {
      agentEventHandler?.(fallbackNoticeEvent(SESSION_ID, 1));
    });

    const notices = result.current.messages.filter(
      (m) => m.metadata?.fallbackNotice != null,
    );
    expect(notices).toHaveLength(1);
    const notice = notices[0];
    expect(notice.role).toBe('system');
    expect(notice.sessionId).toBe(SESSION_ID);
    expect(notice.metadata?.fallbackNotice).toMatchObject({
      statusCode: 400,
      fallbackModel: 'fallback-model',
      errorMessage: 'LLM provider returned HTTP 400: too large',
    });
    const text = notice.content[0];
    expect((text as { text: string }).text).toContain('HTTP 400');
    expect((text as { text: string }).text).toContain('fallback-model');
  });

  it('ignores a fallback notice that belongs to another session', async () => {
    const { result } = renderSessionEvents();
    await waitFor(() => {
      expect(agentEventHandler).toBeDefined();
    });

    act(() => {
      agentEventHandler?.(fallbackNoticeEvent('other-session', 1));
    });

    expect(
      result.current.messages.filter((m) => m.metadata?.fallbackNotice != null),
    ).toHaveLength(0);
  });
});

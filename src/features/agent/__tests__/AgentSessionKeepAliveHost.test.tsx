import { render, waitFor, act } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  MemoryRouter,
  Route,
  Routes,
  useNavigate,
  useParams,
} from 'react-router-dom';
import { useEffect } from 'react';

import {
  createOpenSessionResponse,
  listenMock,
  mockMarkSessionViewed,
  mockRefreshCompactedRange,
  openAgentSessionMock,
  safeInvokeMock,
} from '@/context/__tests__/agent-session-test-utils';
import { clearOpenSessionViewCache } from '@/context/agent-session/openSessionViewCache';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { AgentSessionKeepAliveHost } from '../AgentSessionKeepAliveHost';

vi.mock('@/features/agent/AgentChatView', () => ({
  default: function MockAgentChatView() {
    const state = useAgentSessionState();
    return (
      <div data-testid="chat-view" data-session-id={state.session?.id ?? ''}>
        {state.session?.id}
      </div>
    );
  },
}));

function KeepAliveRoute() {
  const { sessionId } = useParams<{ sessionId: string }>();
  if (!sessionId) {
    return null;
  }
  return <AgentSessionKeepAliveHost activeSessionId={sessionId} />;
}

function NavigatingHarness({
  onNavigateReady,
}: {
  onNavigateReady: (navigate: ReturnType<typeof useNavigate>) => void;
}) {
  const navigate = useNavigate();

  useEffect(() => {
    onNavigateReady(navigate);
  }, [navigate, onNavigateReady]);

  return (
    <Routes>
      <Route path="/agent/:sessionId" element={<KeepAliveRoute />} />
    </Routes>
  );
}

describe('AgentSessionKeepAliveHost', () => {
  const mockUnlisten = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    clearOpenSessionViewCache();
    mockMarkSessionViewed.mockResolvedValue(undefined);
    mockRefreshCompactedRange.mockResolvedValue(undefined);
    listenMock.mockResolvedValue(mockUnlisten);
    openAgentSessionMock.mockImplementation(async (sessionId: string) =>
      createOpenSessionResponse(sessionId),
    );
    safeInvokeMock.mockResolvedValue(undefined);
  });

  it('does not re-open a retained session when switching back', async () => {
    let navigate: ReturnType<typeof useNavigate> | undefined;

    render(
      <MemoryRouter initialEntries={['/agent/session-1']}>
        <NavigatingHarness
          onNavigateReady={(nav) => {
            navigate = nav;
          }}
        />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(openAgentSessionMock).toHaveBeenCalledWith('session-1');
      expect(navigate).toBeDefined();
    });

    await act(async () => {
      navigate?.('/agent/session-2');
    });

    await waitFor(() => {
      expect(openAgentSessionMock).toHaveBeenCalledWith('session-2');
    });

    await act(async () => {
      navigate?.('/agent/session-1');
    });

    await waitFor(() => {
      expect(
        document
          .querySelector('[data-testid="chat-view"]')
          ?.getAttribute('data-session-id'),
      ).toBe('session-1');
    });

    expect(
      openAgentSessionMock.mock.calls.filter(([id]) => id === 'session-1'),
    ).toHaveLength(1);
  });
});

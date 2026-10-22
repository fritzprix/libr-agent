import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { AgentSessionProvider } from '@/context/AgentSessionContext';
import type { SessionGoneReason } from '@/context/AgentSessionContext';
import AgentChatView from './AgentChatView';
import { useRetainedSessionIds } from './hooks/useRetainedSessionIds';

interface AgentSessionKeepAliveHostProps {
  activeSessionId: string;
}

/**
 * Keeps the last few AgentSessionProviders mounted so switching back to a
 * recently viewed session does not re-run openAgentSession / hydrating UX.
 *
 * Only the active session mounts AgentChatView (panels, virtuoso, chat UI).
 * Inactive providers still receive agent:event updates in the background.
 *
 * Deleted / missing sessions are evicted so they are never re-opened. A
 * deleted active session leaves the route; a missing active session keeps the
 * not-found view instead of retrying.
 */
export function AgentSessionKeepAliveHost({
  activeSessionId,
}: AgentSessionKeepAliveHostProps) {
  const { retainedIds, evictSessionId } =
    useRetainedSessionIds(activeSessionId);
  const navigate = useNavigate();

  const handleSessionGone = useCallback(
    (sessionId: string, reason: SessionGoneReason) => {
      if (sessionId !== activeSessionId) {
        evictSessionId(sessionId);
        return;
      }
      if (reason === 'deleted') {
        navigate('/agent', { replace: true });
      }
    },
    [activeSessionId, evictSessionId, navigate],
  );

  return (
    <div className="h-full">
      {retainedIds.map((sessionId) => {
        const isActive = sessionId === activeSessionId;
        return (
          <AgentSessionProvider
            key={sessionId}
            sessionId={sessionId}
            isActive={isActive}
            onSessionGone={handleSessionGone}
          >
            {isActive ? <AgentChatView /> : null}
          </AgentSessionProvider>
        );
      })}
    </div>
  );
}

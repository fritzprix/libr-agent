import { AgentSessionProvider } from '@/context/AgentSessionContext';
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
 */
export function AgentSessionKeepAliveHost({
  activeSessionId,
}: AgentSessionKeepAliveHostProps) {
  const retainedIds = useRetainedSessionIds(activeSessionId);

  return (
    <div className="h-full">
      {retainedIds.map((sessionId) => {
        const isActive = sessionId === activeSessionId;
        return (
          <AgentSessionProvider
            key={sessionId}
            sessionId={sessionId}
            isActive={isActive}
          >
            {isActive ? <AgentChatView /> : null}
          </AgentSessionProvider>
        );
      })}
    </div>
  );
}

import { useParams } from 'react-router-dom';
import { AgentSessionProvider } from '@/context/AgentSessionContext';
import AgentChatView from './AgentChatView';

/**
 * Session route for /agent/:sessionId.
 *
 * Kept separate from the start route so the home screen does not eagerly load
 * the full agent session UI and provider graph.
 */
export default function AgentSessionRoute() {
  const { sessionId } = useParams<{ sessionId?: string }>();

  if (!sessionId) {
    return null;
  }

  return (
    <AgentSessionProvider sessionId={sessionId} key={sessionId}>
      <AgentChatView />
    </AgentSessionProvider>
  );
}

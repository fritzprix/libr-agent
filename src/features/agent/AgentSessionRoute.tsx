import { useParams } from 'react-router-dom';
import { AgentSessionKeepAliveHost } from './AgentSessionKeepAliveHost';

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

  return <AgentSessionKeepAliveHost activeSessionId={sessionId} />;
}

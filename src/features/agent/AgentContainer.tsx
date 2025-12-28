import { useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from '@/context/AgentSessionContext';
import AgentChatView from './AgentChatView';
import StartAgentView from './StartAgentView';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentContainer');

/**
 * Agent V2 Container
 *
 * Smart container that routes between start view and chat view
 * based on URL params and agent session state.
 *
 * Routes:
 * - /agent → StartAgentView (select assistant)
 * - /agent/:sessionId → AgentChatView (resume or continue session)
 *
 * Pattern: Mirrors V1's ChatContainer.tsx for consistency
 */
export default function AgentContainer() {
  const { sessionId } = useParams<{ sessionId?: string }>();
  const navigate = useNavigate();
  const { currentSession, isLoading } = useAgentSessionState();
  const { resumeSession, clearSession } = useAgentSessionActions();

  /**
   * Resume session when sessionId is in URL
   */
  useEffect(() => {
    // If URL has sessionId but no current session, try to resume
    if (sessionId && (!currentSession || currentSession.id !== sessionId)) {
      logger.info('Resuming session from URL', { sessionId });

      resumeSession(sessionId).catch((err) => {
        logger.error('Failed to resume session', err);
        // Navigate back to start view on error
        navigate('/agent', { replace: true });
      });
    }

    // If no sessionId in URL but we have a current session, clear it
    if (!sessionId && currentSession) {
      logger.info('No sessionId in URL, clearing current session');
      clearSession();
    }
  }, [sessionId, currentSession?.id, resumeSession, clearSession, navigate]);

  /**
   * Show loading state during session resume
   */
  if (sessionId && isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-muted-foreground">Loading session...</div>
      </div>
    );
  }

  /**
   * Route decision:
   * - If sessionId in URL and session loaded → AgentChatView
   * - Otherwise → StartAgentView
   */
  return sessionId && currentSession?.id === sessionId ? (
    <AgentChatView />
  ) : (
    <StartAgentView />
  );
}

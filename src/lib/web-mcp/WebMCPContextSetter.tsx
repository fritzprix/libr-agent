import { useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';

const logger = getLogger('WebMCPContextSetter');

/**
 * Headless component that sets context for WebMCP servers on mount.
 * This component automatically configures session and assistant contexts
 * for planning and playbook servers based on current session and assistant state.
 */
export function WebMCPContextSetter() {
  const { getCurrentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();

  // Get server proxies
  const { server: planningServer, loading: planningLoading } =
    useWebMCPServer('planning');
  const { server: playbookServer, loading: playbookLoading } =
    useWebMCPServer('playbook');

  // Get current session once so we can depend on its id in the effect deps.
  const currentSession = getCurrentSession();
  const currentSessionId = currentSession?.id;

  // Set context for planning server when session changes
  useEffect(() => {
    if (!planningLoading && planningServer?.setContext && currentSessionId) {
      planningServer
        .setContext({ sessionId: currentSessionId })
        .then(() => {
          logger.debug('Planning server context set', {
            sessionId: currentSessionId,
          });
        })
        .catch((error) => {
          logger.error('Failed to set planning server context', {
            sessionId: currentSessionId,
            error,
          });
        });
    }
  }, [planningServer, planningLoading, currentSessionId, getCurrentSession]);

  // Set context for playbook server when assistant changes
  useEffect(() => {
    if (
      !playbookLoading &&
      playbookServer?.setContext &&
      currentAssistant?.id
    ) {
      playbookServer
        .setContext({ assistantId: currentAssistant.id })
        .then(() => {
          logger.debug('Playbook server context set', {
            assistantId: currentAssistant.id,
          });
        })
        .catch((error) => {
          logger.error('Failed to set playbook server context', {
            assistantId: currentAssistant.id,
            error,
          });
        });
    }
  }, [playbookServer, playbookLoading, currentAssistant]);

  // This component renders nothing - it's purely for side effects
  return null;
}

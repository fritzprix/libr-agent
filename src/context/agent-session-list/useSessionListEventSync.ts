import { useEffect, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { matchPath, useLocation } from 'react-router-dom';
import { useBackendResource } from '@/context/GlobalEventContext';
import { getLogger } from '@/lib/logger';
import type { AgentSession } from '@/models/agent';
import { type AgentEventPayload, handleAgentEvent } from './events';

const logger = getLogger('AgentSessionListContext');
const RESERVED_AGENT_SUBROUTES = new Set(['draft']);

interface SessionListEventSyncParams {
  loadSessions: (forceRefresh?: boolean) => Promise<void>;
  applySessionUpdate: (
    sessionId: string,
    updater: (session: AgentSession) => AgentSession,
  ) => void;
  clearPendingApproval: (sessionId: string, toolCallId: string) => void;
  markSessionViewed: (sessionId: string, viewedAt?: Date) => Promise<void>;
  pendingApprovalKeysRef: { current: Set<string> };
}

/**
 * Keeps the session list in sync with the backend: initial load, resource
 * refreshes, the active route's viewed state, and lightweight agent events.
 */
export function useSessionListEventSync({
  loadSessions,
  applySessionUpdate,
  clearPendingApproval,
  markSessionViewed,
  pendingApprovalKeysRef,
}: SessionListEventSyncParams) {
  const location = useLocation();
  const activeSessionId = useMemo(() => {
    const sessionId = matchPath('/agent/:sessionId', location.pathname)?.params
      .sessionId;
    if (!sessionId || RESERVED_AGENT_SUBROUTES.has(sessionId)) {
      return undefined;
    }
    return sessionId;
  }, [location.pathname]);

  // Initial load
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Subscribe to agent:event for session resource updates via centralized hook
  useBackendResource('session', () => {
    logger.debug('Agent updated session resource, refreshing session list...');
    loadSessions(true);
  });

  useEffect(() => {
    if (!activeSessionId) {
      return;
    }

    const viewedAt = new Date();
    void markSessionViewed(activeSessionId, viewedAt).catch((err) => {
      logger.error('Failed to persist viewed state for active session', err);
    });
  }, [activeSessionId, markSessionViewed]);

  // Subscribe to lightweight agent events to keep session metadata fresh in place.
  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        const cleanup = await listen<AgentEventPayload>(
          'agent:event',
          (event) => {
            if (!isMounted) {
              return;
            }

            handleAgentEvent(
              {
                ...event.payload,
                runtimeState:
                  event.payload.type === 'sessionRuntimeStateUpdated'
                    ? event.payload.runtimeState
                    : undefined,
              },
              {
                activeSessionId,
                applySessionUpdate,
                clearPendingApproval,
                logger,
                markSessionViewed,
                pendingApprovalKeysRef,
              },
            );
          },
        );

        if (isMounted) {
          unlisten = cleanup;
        } else {
          cleanup();
        }
      } catch (err) {
        logger.error('Failed to setup agent session list event listener', err);
      }
    };

    void setup();
    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, [
    activeSessionId,
    applySessionUpdate,
    clearPendingApproval,
    markSessionViewed,
    pendingApprovalKeysRef,
  ]);
}

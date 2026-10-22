import { useCallback, useRef } from 'react';
import { toast } from 'sonner';
import { useLLMService } from '@/context/LLMServiceContext';
import { useModelOptions } from '@/context/ModelProvider';
import { useSettings } from '@/context/SettingsContext';
import { safeInvoke } from '@/lib/backend/core';
import { getAssistant } from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';
import { generateSessionId } from '@/lib/session-id';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type { AgentSession, CreateSessionParams } from '@/models/agent';
import type { AgentResponse, AgentSessionMetadata } from '@/models/agent-ipc';
import { buildCreateAgentSessionRequest, mapCreatedSession } from './mappings';
import type { SessionListStore } from './useSessionListStore';

const logger = getLogger('AgentSessionListContext');

type SessionListMutationsStore = Pick<
  SessionListStore,
  | 'sessionsRef'
  | 'notificationSessionsRef'
  | 'mutateSessions'
  | 'applySessionUpdate'
>;

/**
 * Session create/delete/rename/bookmark/view actions that persist through the
 * backend and keep the in-memory collections in sync.
 */
export function useSessionListMutations({
  sessionsRef,
  notificationSessionsRef,
  mutateSessions,
  applySessionUpdate,
}: SessionListMutationsStore) {
  const { modelId, provider } = useModelOptions();
  const {
    value: { advanced },
  } = useSettings();
  const { clearSessionState } = useLLMService();
  const pendingApprovalKeysRef = useRef(new Set<string>());

  /**
   * Create a new agent session
   */
  const createSession = useCallback(
    async (params: CreateSessionParams): Promise<AgentSession> => {
      const { assistant, name } = params;

      logger.info('Creating new agent session', {
        assistantName: assistant.name,
        sessionName: name,
      });

      try {
        // ✅ CRITICAL FIX: Reload assistant from DB to get latest configuration
        // This ensures that any recent changes (e.g., built-in tool updates)
        // are included in the session config
        const freshAssistant = await getAssistant(assistant.id);

        if (!freshAssistant) {
          throw new Error(`Assistant ${assistant.id} not found in database`);
        }

        logger.debug('Reloaded assistant from DB', {
          assistantId: freshAssistant.id,
          allowedBuiltInServiceAliases:
            freshAssistant.allowedBuiltInServiceAliases,
        });

        const request = buildCreateAgentSessionRequest({
          assistant: freshAssistant,
          name,
          modelId,
          provider,
          runtimeLimits: {
            defaultSessionMaxDepth: advanced.defaultSessionMaxDepth,
            defaultSessionMaxFanout: advanced.defaultSessionMaxFanout,
          },
          sessionId: generateSessionId(),
        });

        // Call Rust backend to create session
        const response = await safeInvoke<AgentSessionMetadata>(
          'agent_create_session',
          { request },
        );

        const session = mapCreatedSession(response, freshAssistant);

        // Add to list
        mutateSessions((prev) => [session, ...prev]);

        toast.success(
          `Session "${session.name || session.id.slice(0, 8)}" created successfully`,
        );

        logger.info('Agent session created successfully', {
          sessionId: session.id,
        });

        return session;
      } catch (err) {
        logger.error('Failed to create agent session', err);
        throw err;
      }
    },
    [
      advanced.defaultSessionMaxDepth,
      advanced.defaultSessionMaxFanout,
      modelId,
      mutateSessions,
      provider,
    ],
  );

  /**
   * Delete an agent session (cascade: removes all descendants too)
   */
  const deleteSession = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting agent session', { sessionId });

      try {
        const response = await safeInvoke<AgentResponse<string[]>>(
          'agent_delete_session',
          { sessionId },
        );
        const data = response?.data;
        let deletedIds: string[];

        if (
          Array.isArray(data) &&
          data.every((x): x is string => typeof x === 'string')
        ) {
          deletedIds = data;
        } else {
          logger.warn(
            'agent_delete_session returned unexpected data; falling back to single id',
            {
              sessionId,
              data,
            },
          );
          deletedIds = [sessionId];
        }

        const idsToRemove = new Set(deletedIds);

        // Remove the session and ALL its descendants from the UI using the authoritative list from Rust
        mutateSessions((prev) => prev.filter((s) => !idsToRemove.has(s.id)), {
          notificationUpdater: (prev) =>
            prev.filter((s) => !idsToRemove.has(s.id)),
        });

        // Clean up LLM state outside the updater to avoid double-invocation in StrictMode
        idsToRemove.forEach((id) => clearSessionState(id));

        logger.info('Session deleted successfully', { sessionId, deletedIds });
      } catch (err) {
        logger.error('Failed to delete session', err);
        throw err;
      }
    },
    [clearSessionState, mutateSessions],
  );

  /**
   * Delete only this session, orphaning direct children as top-level sessions
   */
  const deleteSessionOnly = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting session only (orphaning children)', { sessionId });

      try {
        const response = await safeInvoke<
          AgentResponse<{ deletedId: string; orphanedIds: string[] }>
        >('agent_delete_session_only', {
          sessionId,
        });

        const actualDeletedId = response?.data?.deletedId || sessionId;
        clearSessionState(actualDeletedId);

        const orphanedIds = new Set(response?.data?.orphanedIds || []);

        // Remove the session; update explicitly orphaned children to have no parent
        mutateSessions(
          (prev) =>
            prev
              .filter((s) => s.id !== actualDeletedId)
              .map((s) =>
                orphanedIds.has(s.id)
                  ? { ...s, parentSessionId: undefined }
                  : s,
              ),
          {
            notificationUpdater: (prev) =>
              prev
                .filter((s) => s.id !== actualDeletedId)
                .map((s) =>
                  orphanedIds.has(s.id)
                    ? { ...s, parentSessionId: undefined }
                    : s,
                ),
          },
        );

        logger.info('Session deleted (children orphaned)', { sessionId });
      } catch (err) {
        logger.error('Failed to delete session only', err);
        throw err;
      }
    },
    [clearSessionState, mutateSessions],
  );

  /**
   * Toggle the bookmark flag on a session (optimistic update)
   */
  const toggleBookmark = useCallback(
    async (sessionId: string, currentBookmarked?: boolean) => {
      const session =
        sessionsRef.current.find((candidate) => candidate.id === sessionId) ??
        notificationSessionsRef.current.find(
          (candidate) => candidate.id === sessionId,
        );
      const previousValue = currentBookmarked ?? session?.isBookmarked ?? false;
      const newValue = !previousValue;

      mutateSessions(
        (prev) =>
          prev.map((s) =>
            s.id === sessionId ? { ...s, isBookmarked: newValue } : s,
          ),
        {
          notificationUpdater: (prev) =>
            prev.map((s) =>
              s.id === sessionId ? { ...s, isBookmarked: newValue } : s,
            ),
        },
      );

      try {
        await safeInvoke<void>('agent_toggle_session_bookmark', {
          sessionId,
          bookmarked: newValue,
        });
      } catch (err) {
        logger.error('Failed to toggle bookmark', err);
        applySessionUpdate(sessionId, (session) => ({
          ...session,
          isBookmarked: previousValue,
        }));
        throw err;
      }
    },
    [applySessionUpdate, mutateSessions, notificationSessionsRef, sessionsRef],
  );

  const renameSession = useCallback(
    async (sessionId: string, name: string) => {
      const normalizedName = name.trim();
      if (!normalizedName) {
        throw new Error('Session title cannot be empty');
      }

      const currentSession =
        sessionsRef.current.find((session) => session.id === sessionId) ??
        notificationSessionsRef.current.find(
          (session) => session.id === sessionId,
        );
      const previousName = currentSession?.name;

      if (previousName === normalizedName) {
        return;
      }

      mutateSessions(
        (previousSessions) =>
          previousSessions.map((session) =>
            session.id === sessionId
              ? { ...session, name: normalizedName }
              : session,
          ),
        {
          notificationUpdater: (previousNotifications) =>
            previousNotifications.map((session) =>
              session.id === sessionId
                ? { ...session, name: normalizedName }
                : session,
            ),
        },
      );

      try {
        await safeInvoke<void>('agent_update_session_name', {
          sessionId,
          name: normalizedName,
        });
      } catch (err) {
        logger.error('Failed to rename session', err);
        applySessionUpdate(sessionId, (session) => ({
          ...session,
          name: previousName,
        }));
        throw err;
      }
    },
    [applySessionUpdate, mutateSessions, notificationSessionsRef, sessionsRef],
  );

  const markSessionViewed = useCallback(
    async (sessionId: string, viewedAt = new Date()) => {
      await safeInvoke<void>('agent_mark_session_viewed', {
        sessionId,
        viewedAt: viewedAt.getTime(),
      });
      applySessionUpdate(sessionId, (session) =>
        applyViewedAtToSession(session, viewedAt),
      );
    },
    [applySessionUpdate],
  );

  const clearPendingApproval = useCallback(
    (sessionId: string, toolCallId: string) => {
      const pendingApprovalKey = `${sessionId}:${toolCallId}`;
      if (!pendingApprovalKeysRef.current.delete(pendingApprovalKey)) {
        return;
      }
      applySessionUpdate(sessionId, (session) => ({
        ...session,
        pendingApprovalCount: Math.max(
          0,
          (session.pendingApprovalCount ?? 0) - 1,
        ),
      }));
    },
    [applySessionUpdate],
  );

  return {
    pendingApprovalKeysRef,
    createSession,
    deleteSession,
    deleteSessionOnly,
    toggleBookmark,
    renameSession,
    markSessionViewed,
    clearPendingApproval,
  };
}

import { useCallback, useRef, useState } from 'react';
import type { AgentSession } from '@/models/agent';
import {
  applySessionUpdateToCollections,
  pruneNotificationSessions,
} from './collections';
import { invalidateSessionListStartupCache } from './startup-cache';

export interface MutateSessionsOptions {
  notificationUpdater?: (
    previousNotifications: AgentSession[],
  ) => AgentSession[];
}

/**
 * Owns the session and notification collections plus the refs that mirror
 * them, so async callbacks can read the latest lists without re-subscribing.
 */
export function useSessionListStore() {
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [notificationSessions, setNotificationSessions] = useState<
    AgentSession[]
  >([]);
  const sessionsRef = useRef<AgentSession[]>([]);
  const notificationSessionsRef = useRef<AgentSession[]>([]);
  const sessionListMutationVersionRef = useRef(0);
  sessionsRef.current = sessions;
  notificationSessionsRef.current = notificationSessions;

  const mutateSessions = useCallback(
    (
      updater: (previousSessions: AgentSession[]) => AgentSession[],
      options?: MutateSessionsOptions,
    ) => {
      sessionListMutationVersionRef.current += 1;
      invalidateSessionListStartupCache();
      setSessions((previousSessions) => {
        const nextSessions = updater(previousSessions);
        sessionsRef.current = nextSessions;
        return nextSessions;
      });
      if (options?.notificationUpdater) {
        setNotificationSessions((previousNotifications) => {
          const nextNotifications = pruneNotificationSessions(
            options.notificationUpdater?.(previousNotifications) ??
              previousNotifications,
          );
          notificationSessionsRef.current = nextNotifications;
          return nextNotifications;
        });
      }
    },
    [],
  );

  const applySessionUpdate = useCallback(
    (sessionId: string, updater: (session: AgentSession) => AgentSession) => {
      const nextCollections = applySessionUpdateToCollections({
        sessions: sessionsRef.current,
        notificationSessions: notificationSessionsRef.current,
        sessionId,
        updater,
      });

      sessionsRef.current = nextCollections.sessions;
      notificationSessionsRef.current = nextCollections.notificationSessions;
      setSessions(nextCollections.sessions);
      setNotificationSessions(nextCollections.notificationSessions);
    },
    [],
  );

  return {
    sessions,
    setSessions,
    notificationSessions,
    setNotificationSessions,
    sessionsRef,
    notificationSessionsRef,
    sessionListMutationVersionRef,
    mutateSessions,
    applySessionUpdate,
  };
}

export type SessionListStore = ReturnType<typeof useSessionListStore>;

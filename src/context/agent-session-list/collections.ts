import type { AgentSession } from '@/models/agent';

export interface SessionCollections {
  sessions: AgentSession[];
  notificationSessions: AgentSession[];
}

export function dedupeSessionsById<T extends AgentSession>(sessions: T[]): T[] {
  const sessionById = new Map<string, T>();
  sessions.forEach((session) => {
    sessionById.set(session.id, session);
  });
  return Array.from(sessionById.values());
}

export function hasUnreadAttention(session: AgentSession): boolean {
  if (!session.lastAttentionAt || !session.lastAttentionReason) {
    return false;
  }

  if (!session.lastViewedAt) {
    return true;
  }

  return session.lastAttentionAt.getTime() > session.lastViewedAt.getTime();
}

function getNotificationTimestamp(session: AgentSession): number {
  return (
    session.lastAttentionAt?.getTime() ??
    session.lastMessageAt?.getTime() ??
    session.updatedAt?.getTime() ??
    session.createdAt.getTime()
  );
}

export function sortNotificationSessions(
  items: AgentSession[],
): AgentSession[] {
  return items.slice().sort((left, right) => {
    const leftPending = left.pendingApprovalCount ?? 0;
    const rightPending = right.pendingApprovalCount ?? 0;
    if (leftPending !== rightPending) {
      return rightPending - leftPending;
    }

    return getNotificationTimestamp(right) - getNotificationTimestamp(left);
  });
}

export function pruneNotificationSessions(
  items: AgentSession[],
): AgentSession[] {
  const dedupedSessions = dedupeSessionsById(items);
  const unreadSessions = dedupedSessions.filter((session) =>
    hasUnreadAttention(session),
  );
  return sortNotificationSessions(unreadSessions);
}

export function updateSessionInList(
  items: AgentSession[],
  sessionId: string,
  updater: (session: AgentSession) => AgentSession,
): AgentSession[] {
  return items.map((session) =>
    session.id === sessionId ? updater(session) : session,
  );
}

function mergeSessionIntoNotifications(
  previousNotifications: AgentSession[],
  sessionId: string,
  nextSession: AgentSession,
): AgentSession[] {
  const existingNotification = previousNotifications.some(
    (session) => session.id === sessionId,
  );

  if (existingNotification) {
    return pruneNotificationSessions(
      previousNotifications.map((session) =>
        session.id === sessionId ? nextSession : session,
      ),
    );
  }

  if (!hasUnreadAttention(nextSession)) {
    return previousNotifications;
  }

  return pruneNotificationSessions([...previousNotifications, nextSession]);
}

export function applySessionUpdateToCollections(args: {
  sessions: AgentSession[];
  notificationSessions: AgentSession[];
  sessionId: string;
  updater: (session: AgentSession) => AgentSession;
}): SessionCollections {
  const { notificationSessions, sessionId, sessions, updater } = args;
  const sessionFromSessions = sessions.find(
    (session) => session.id === sessionId,
  );
  const sessionFromNotifications = notificationSessions.find(
    (session) => session.id === sessionId,
  );
  const baseSession = sessionFromSessions ?? sessionFromNotifications;

  if (!baseSession) {
    return {
      sessions,
      notificationSessions,
    };
  }

  const nextSession = updater(baseSession);

  return {
    sessions: sessionFromSessions
      ? updateSessionInList(sessions, sessionId, () => nextSession)
      : sessions,
    notificationSessions: mergeSessionIntoNotifications(
      notificationSessions,
      sessionId,
      nextSession,
    ),
  };
}

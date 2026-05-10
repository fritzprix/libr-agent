import { applyViewedAtToSession } from '@/lib/session-utils';
import type { AgentSession } from '@/models/agent';
import type { WorkflowCompletionReason } from '@/models/agent-ipc';

export interface AgentEventPayload {
  type: string;
  sessionId?: string;
  status?: AgentSession['status'];
  message?: {
    role: 'user' | 'assistant' | 'system' | 'tool';
    createdAt: number;
  };
  toolCallId?: string;
  approvalKind?: 'standard' | 'hard';
  approved?: boolean;
  reason?: WorkflowCompletionReason;
}

interface EventLogger {
  error: (message: string, ...args: unknown[]) => unknown;
}

interface EventHandlerDeps {
  activeSessionId?: string;
  applySessionUpdate: (
    sessionId: string,
    updater: (session: AgentSession) => AgentSession,
  ) => void;
  clearPendingApproval: (sessionId: string, toolCallId: string) => void;
  logger: EventLogger;
  markSessionViewed: (sessionId: string, viewedAt?: Date) => Promise<void>;
  pendingApprovalKeysRef: { current: Set<string> };
}

type AgentEventHandler = (
  payload: AgentEventPayload,
  deps: EventHandlerDeps,
) => void;

function applyViewedAttentionUpdate(args: {
  activeSessionId?: string;
  applySessionUpdate: EventHandlerDeps['applySessionUpdate'];
  attentionAt: Date;
  logger: EventLogger;
  markSessionViewed: EventHandlerDeps['markSessionViewed'];
  onSessionUpdate: (session: AgentSession) => AgentSession;
  sessionId: string;
  viewedErrorMessage: string;
}) {
  const {
    activeSessionId,
    applySessionUpdate,
    attentionAt,
    logger,
    markSessionViewed,
    onSessionUpdate,
    sessionId,
    viewedErrorMessage,
  } = args;
  const shouldMarkViewed = sessionId === activeSessionId;

  applySessionUpdate(sessionId, (session) => {
    const nextSession = onSessionUpdate(session);
    return shouldMarkViewed
      ? applyViewedAtToSession(nextSession, attentionAt)
      : nextSession;
  });

  if (shouldMarkViewed) {
    void markSessionViewed(sessionId, attentionAt).catch((error) => {
      logger.error(viewedErrorMessage, error);
    });
  }
}

const handleStatusChanged: AgentEventHandler = (payload, deps) => {
  const nextStatus = payload.status;
  if (!payload.sessionId || nextStatus === undefined) {
    return;
  }

  deps.applySessionUpdate(payload.sessionId, (session) => ({
    ...session,
    status: nextStatus,
  }));
};

const handleMessageAdded: AgentEventHandler = (payload, deps) => {
  if (!payload.sessionId || !payload.message) {
    return;
  }

  const messageAt = new Date(payload.message.createdAt);
  const shouldMarkViewed = payload.sessionId === deps.activeSessionId;

  deps.applySessionUpdate(payload.sessionId, (session) => ({
    ...session,
    lastMessageAt: messageAt,
    lastViewedAt: shouldMarkViewed ? messageAt : session.lastViewedAt,
  }));
};

const handleWorkflowCompleted: AgentEventHandler = (payload, deps) => {
  if (!payload.sessionId || payload.reason !== 'recurringStop') {
    return;
  }

  const attentionAt = new Date();
  applyViewedAttentionUpdate({
    activeSessionId: deps.activeSessionId,
    applySessionUpdate: deps.applySessionUpdate,
    attentionAt,
    logger: deps.logger,
    markSessionViewed: deps.markSessionViewed,
    onSessionUpdate: (session) => ({
      ...session,
      lastAttentionAt: attentionAt,
      lastAttentionReason: 'recurringStop',
    }),
    sessionId: payload.sessionId,
    viewedErrorMessage:
      'Failed to mark active session viewed after recurring stop',
  });
};

const handleToolExecutionRequiresApproval: AgentEventHandler = (
  payload,
  deps,
) => {
  if (!payload.sessionId || !payload.toolCallId) {
    return;
  }

  const pendingApprovalKey = `${payload.sessionId}:${payload.toolCallId}`;
  if (deps.pendingApprovalKeysRef.current.has(pendingApprovalKey)) {
    return;
  }

  deps.pendingApprovalKeysRef.current.add(pendingApprovalKey);
  const attentionAt = new Date();
  applyViewedAttentionUpdate({
    activeSessionId: deps.activeSessionId,
    applySessionUpdate: deps.applySessionUpdate,
    attentionAt,
    logger: deps.logger,
    markSessionViewed: deps.markSessionViewed,
    onSessionUpdate: (session) => ({
      ...session,
      lastAttentionAt: attentionAt,
      lastAttentionReason: 'pendingApproval',
      pendingApprovalCount: (session.pendingApprovalCount ?? 0) + 1,
    }),
    sessionId: payload.sessionId,
    viewedErrorMessage:
      'Failed to mark active session viewed after approval request',
  });
};

const handleToolExecutionApprovalResolved: AgentEventHandler = (
  payload,
  deps,
) => {
  if (!payload.sessionId || !payload.toolCallId) {
    return;
  }

  deps.clearPendingApproval(payload.sessionId, payload.toolCallId);
};

const EVENT_HANDLERS: Record<string, AgentEventHandler> = {
  statusChanged: handleStatusChanged,
  messageAdded: handleMessageAdded,
  workflowCompleted: handleWorkflowCompleted,
  toolExecutionRequiresApproval: handleToolExecutionRequiresApproval,
  toolExecutionApprovalResolved: handleToolExecutionApprovalResolved,
};

export function handleAgentEvent(
  payload: AgentEventPayload,
  deps: EventHandlerDeps,
) {
  const handler = EVENT_HANDLERS[payload.type];
  if (!handler) {
    return;
  }

  handler(payload, deps);
}

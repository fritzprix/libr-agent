import type { Message } from '@/models/chat';
import type { PendingApproval } from './types';

/**
 * Reconcile DB open-slice with messages that arrived via agent:event while
 * openAgentSession was in flight. Keep event-only ids and live streaming rows.
 */
export function mergeOpenSessionMessages(
  previous: Message[],
  incoming: Message[],
): Message[] {
  if (previous.length === 0) {
    return incoming;
  }
  if (incoming.length === 0) {
    return previous;
  }

  const byId = new Map<string, Message>();
  for (const message of incoming) {
    byId.set(message.id, message);
  }
  for (const message of previous) {
    const existing = byId.get(message.id);
    if (!existing) {
      byId.set(message.id, message);
      continue;
    }
    if (message.isStreaming && !existing.isStreaming) {
      byId.set(message.id, message);
    }
  }

  return Array.from(byId.values()).sort((left, right) => {
    const leftTime = left.createdAt?.getTime() ?? 0;
    const rightTime = right.createdAt?.getTime() ?? 0;
    if (leftTime !== rightTime) {
      return leftTime - rightTime;
    }
    return left.id.localeCompare(right.id);
  });
}

/**
 * Keep approvals observed via events that the open snapshot has not caught yet.
 */
export function mergePendingApprovals(
  previous: PendingApproval[],
  incoming: PendingApproval[],
): PendingApproval[] {
  if (previous.length === 0) {
    return incoming;
  }
  if (incoming.length === 0) {
    return previous;
  }

  const byId = new Map<string, PendingApproval>();
  for (const approval of incoming) {
    byId.set(approval.toolCallId, approval);
  }
  for (const approval of previous) {
    if (!byId.has(approval.toolCallId)) {
      byId.set(approval.toolCallId, approval);
    }
  }
  return Array.from(byId.values());
}

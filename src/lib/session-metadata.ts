import type { AgentSession } from '@/models/agent';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import type { Assistant } from '@/models/chat';
import { normalizeExecutionMode } from '@/lib/generated/execution-mode';

export { normalizeExecutionMode } from '@/lib/generated/execution-mode';

export function mapSessionMetadataToAgentSession(
  metadata: AgentSessionMetadata,
  pendingApprovalCount = 0,
  assistant?: Assistant,
): AgentSession {
  const parentSessionId = metadata.parentSessionId;
  const lineageId = metadata.lineageId ?? parentSessionId ?? metadata.id;
  const depth = metadata.depth ?? (parentSessionId ? 1 : 0);
  const executionMode = normalizeExecutionMode(metadata.executionMode);

  return {
    id: metadata.id,
    name: metadata.name,
    status: metadata.status,
    model: metadata.model,
    provider: metadata.provider,
    assistant,
    parentSessionId,
    lineageId,
    depth,
    orgId: metadata.orgId,
    orgName: metadata.orgName,
    orgRootSessionId: metadata.orgRootSessionId,
    createdAt: new Date(metadata.createdAt),
    updatedAt: metadata.updatedAt ? new Date(metadata.updatedAt) : undefined,
    lastViewedAt: metadata.lastViewedAt
      ? new Date(metadata.lastViewedAt)
      : undefined,
    lastMessageAt: metadata.lastMessageAt
      ? new Date(metadata.lastMessageAt)
      : undefined,
    lastAttentionAt: metadata.lastAttentionAt
      ? new Date(metadata.lastAttentionAt)
      : undefined,
    lastAttentionReason: metadata.lastAttentionReason,
    isBookmarked: metadata.isBookmarked ?? false,
    executionMode,
    pendingApprovalCount,
  };
}

export function getLatestSessionActivityTimestamp(
  session: AgentSession,
): number {
  return (
    session.lastMessageAt?.getTime() ??
    session.updatedAt?.getTime() ??
    session.createdAt.getTime()
  );
}

export function sortSessionsByLatestActivity(
  sessions: AgentSession[],
): AgentSession[] {
  return sessions
    .slice()
    .sort(
      (left, right) =>
        getLatestSessionActivityTimestamp(right) -
        getLatestSessionActivityTimestamp(left),
    );
}

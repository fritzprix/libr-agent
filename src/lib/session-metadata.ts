import type { AgentSession } from '@/models/agent';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import type { Assistant } from '@/models/chat';

export function coalesceExecutionModeFlags(
  yoloMode: boolean | undefined,
  unsafeMode: boolean | undefined,
): {
  executionMode: 'normal' | 'yolo' | 'unsafe';
  yoloMode: boolean;
  unsafeMode: boolean;
} {
  const normalizedUnsafeMode = unsafeMode === true;
  const normalizedYoloMode = normalizedUnsafeMode ? false : yoloMode === true;

  return {
    executionMode: normalizedUnsafeMode
      ? 'unsafe'
      : normalizedYoloMode
        ? 'yolo'
        : 'normal',
    yoloMode: normalizedYoloMode,
    unsafeMode: normalizedUnsafeMode,
  };
}

export function mapSessionMetadataToAgentSession(
  metadata: AgentSessionMetadata,
  pendingApprovalCount = 0,
  assistant?: Assistant,
): AgentSession {
  const parentSessionId = metadata.parentSessionId;
  const lineageId = metadata.lineageId ?? parentSessionId ?? metadata.id;
  const depth = metadata.depth ?? (parentSessionId ? 1 : 0);
  const executionMode = coalesceExecutionModeFlags(
    metadata.yoloMode,
    metadata.unsafeMode,
  );

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
    yoloMode: executionMode.yoloMode,
    unsafeMode: executionMode.unsafeMode,
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

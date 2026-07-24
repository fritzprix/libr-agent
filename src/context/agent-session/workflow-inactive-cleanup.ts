import type { Message } from '@/models/chat';

/** Agent session / workflow statuses that mean "not actively running". */
export type AgentWorkflowStatus =
  | 'idle'
  | 'busy'
  | 'paused'
  | 'error'
  | 'queued'
  | 'provisioning';

/**
 * Statuses where ThinkingBubble / streaming placeholders must not remain active.
 * List UI may already show idle/paused while local streaming state lags behind.
 */
export function isInactiveWorkflowStatus(status: AgentWorkflowStatus): boolean {
  return status === 'idle' || status === 'paused' || status === 'error';
}

/**
 * Clear `isStreaming` on persisted/in-memory session messages (idempotent).
 */
export function stripMessageStreamingFlags(messages: Message[]): Message[] {
  if (!messages.some((message) => message.isStreaming)) {
    return messages;
  }
  return messages.map((message) =>
    message.isStreaming ? { ...message, isStreaming: false } : message,
  );
}

export interface WorkflowInactiveCleanupArgs {
  sessionId: string;
  /** Clear LLMServiceContext streaming placeholder for this session. */
  clearStreamingMessage: (sessionId: string) => void;
  /** Update AgentSessionContext message list. */
  setMessages: (updater: (prev: Message[]) => Message[]) => void;
}

/**
 * Single cleanup entry for workflow becoming inactive.
 *
 * Clears both:
 * 1. LLM in-flight streaming placeholder (`streamingMessages` map)
 * 2. Stuck `message.isStreaming` flags in the session message list
 *
 * Call from Rust-driven transitions (statusChanged, workflowCompleted) and
 * when hydrating an already-inactive session. Manual cancel may still call
 * `clearStreamingMessage` immediately for snappier UI; this remains safe to
 * invoke again when the matching status event arrives.
 */
export function applyWorkflowInactiveCleanup({
  sessionId,
  clearStreamingMessage,
  setMessages,
}: WorkflowInactiveCleanupArgs): void {
  clearStreamingMessage(sessionId);
  setMessages((prev) => stripMessageStreamingFlags(prev));
}

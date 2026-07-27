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
 * Soft cleanup when the workflow becomes inactive.
 *
 * Clears:
 * 1. LLM streaming placeholder (`streamingMessages` map)
 * 2. Stuck `message.isStreaming` flags in the session message list
 *
 * Does NOT abort in-flight frontend LLM completions. Aborting here races with
 * legitimate turns (stale idle events / hydrate remounts) and drops the response
 * before Rust receives it. Frontend aborts must come from:
 * - manual `cancel()` / `cancelCompletionRequest`
 * - Rust `llm:completion-cancel` (cancel_workflow / terminate_session)
 *
 * `displayMessages` also refuses to render streaming placeholders while inactive,
 * so late chunks cannot resurrect ThinkingBubble after soft clear.
 */
export function applyWorkflowInactiveCleanup({
  sessionId,
  clearStreamingMessage,
  setMessages,
}: WorkflowInactiveCleanupArgs): void {
  clearStreamingMessage(sessionId);
  setMessages((prev) => stripMessageStreamingFlags(prev));
}

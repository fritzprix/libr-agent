import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';

/**
 * Returns true if the error is an intentional abort (user cancel via AbortController).
 * Used to distinguish cancellation from real failures in both execution and listener.
 *
 * Handles both:
 *  - DOMException {name:'AbortError'} thrown by fetch when AbortController fires
 *    (DOMException does not extend Error in some environments such as jsdom)
 *  - Error {message:'Request aborted'} thrown by some LLM SDKs
 */
export function isAbortError(error: unknown): boolean {
  if (error == null || typeof error !== 'object') return false;
  const e = error as Record<string, unknown>;
  return (
    e['name'] === 'AbortError' ||
    (typeof e['message'] === 'string' && e['message'] === 'Request aborted')
  );
}

/**
 * Request from Rust backend to execute an LLM completion
 */
export interface CompletionRequest {
  sessionId: string;
  messages: Message[];
  model: string;
  provider: string;
  apiKey?: string;
  /** Stable system prompt (sections 1–3). Cacheable across turns. */
  systemPrompt?: string;
  /**
   * Volatile session context (sections 4–5: context providers + service tool states).
   * Rebuilt on every LLM call. Each AI service decides how to inject this via
   * `prepareContextInjection` — may append to system prompt (default) or send as
   * an ephemeral message to maximise prefix-cache hit rates.
   */
  sessionContext?: string;
  temperature?: number;
  maxTokens?: number;
  availableTools?: MCPTool[];
  backendOwnedCompaction?: boolean;
}

export interface CompactionRequest {
  requestId: string;
  sessionId: string;
  messages: Message[];
  model: string;
  provider: string;
}

export interface CompactionStateEvent {
  sessionId: string;
  status: 'idle' | 'awaiting' | 'compacting';
  contextUsage?: {
    totalTokens: number;
    contextWindow: number;
    modelMaxContext?: number;
  };
  compactedRange?: {
    fromId: string;
    toId: string;
  };
}

/**
 * Status of LLM execution for a specific session
 */
export type SessionStatus = 'idle' | 'streaming' | 'error';

/**
 * Context value for LLM Service Provider
 */
export interface LLMServiceContextValue {
  /**
   * Map of session IDs to their current streaming messages
   */
  streamingMessages: Map<string, Partial<Message>>;

  /**
   * Get the current status of a session
   */
  getSessionStatus: (sessionId: string) => SessionStatus;

  /**
   * Clear streaming message for a session
   * Called by AgentChatContext after persisting to message stack
   */
  clearStreamingMessage: (sessionId: string) => void;

  /**
   * Execute a completion request for a session
   * This is invoked by Rust via IPC events
   */
  executeCompletionRequest: (
    sessionId: string,
    messages: Message[],
    model: string,
    provider: string,
    apiKey?: string,
    systemPrompt?: string,
    sessionContext?: string,
    temperature?: number,
    maxTokens?: number,
    availableTools?: MCPTool[],
  ) => Promise<Message>;

  /**
   * Cancel an ongoing completion request for a session
   */
  cancelCompletionRequest: (sessionId: string) => void;

  /**
   * Release all in-memory compaction state for a deleted session.
   * Call this whenever a session is permanently removed.
   */
  clearSessionState: (sessionId: string) => void;

  /**
   * Release in-memory compaction state for ALL sessions.
   * Call this when the global context strategy changes (e.g. compact → window)
   * so that stale caches, resolvers, and UI state do not leak across modes.
   */
  clearAllCompactState: () => void;

  /** Returns true if the session is actively running async compaction */
  isCompacting: (sessionId: string) => boolean;

  /** Returns true if the session is blocked waiting for compaction to finish */
  isAwaitingCompact: (sessionId: string) => boolean;

  /** Current context window usage for the session (compact strategy only) */
  getContextUsage: (
    sessionId: string,
  ) =>
    | { totalTokens: number; contextWindow: number; modelMaxContext?: number }
    | undefined;

  /** Compacted message range for the session, used to render a chat divider */
  getCompactedRange: (
    sessionId: string,
  ) => { fromId: string; toId: string } | undefined;
}

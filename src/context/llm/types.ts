import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { CompactionPressure } from '@/models/agent-ipc';

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
  responseMessageId: string;
  messages: Message[];
  model: string;
  provider: string;
  apiKey?: string;
  /** Stable system prompt (base sections plus stable service-context blocks). */
  systemPrompt?: string;
  /**
   * Per-turn session context (context providers + non-stable service tool states).
   * Rebuilt on every LLM call. Each AI service decides how to inject this via
   * `prepareContextInjection`.
   */
  sessionContext?: string;
  temperature?: number;
  maxTokens?: number;
  availableTools?: MCPTool[];
}

export interface CompactionParentRequest {
  model: string;
  provider: string;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
}

export interface CompactRequest {
  sessionId: string;
  sessionName: string;
  messages: Message[];
  fromId: string;
  toId: string;
  parentRequest?: CompactionParentRequest;
  resumeCompletionAfterCompact: boolean;
}

export interface CompactedRange {
  fromId: string;
  toId: string;
  summary?: string;
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

  /** Last post-response compaction pressure emitted by Rust for this session. */
  getCompactionPressure: (sessionId: string) => CompactionPressure | undefined;

  /** Compacted message range for the session, used to render a compaction event card */
  getCompactedRange: (sessionId: string) => CompactedRange | undefined;

  /** Reload persisted compact-context state for a session into frontend memory. */
  refreshCompactedRange: (sessionId: string) => Promise<void>;
}

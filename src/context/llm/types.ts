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
  systemPrompt?: string;
  temperature?: number;
  maxTokens?: number;
  availableTools?: MCPTool[];
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
    temperature?: number,
    maxTokens?: number,
    availableTools?: MCPTool[],
  ) => Promise<Message>;

  /**
   * Set agent mode (auto-tool use) for a session
   */
  setAgentMode: (sessionId: string, enabled: boolean) => void;

  /**
   * Get agent mode status for a session
   */
  getAgentMode: (sessionId: string) => boolean;

  /**
   * Cancel an ongoing completion request for a session
   */
  cancelCompletionRequest: (sessionId: string) => void;
}

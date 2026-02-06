import { ReactNode } from 'react';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp-types';

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
}

export interface LLMServiceProviderProps {
  children: ReactNode;
}

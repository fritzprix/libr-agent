import { Assistant } from './chat';

/**
 * Agent session metadata from Rust backend
 */
export interface AgentSession {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  createdAt: Date;
  updatedAt?: Date;
  assistant?: Assistant;
}

/**
 * Agent configuration for creating a new session
 */
export interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
}

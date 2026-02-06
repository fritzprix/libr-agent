import { Assistant } from './chat';

/**
 * Runtime agent configuration (includes session-specific settings)
 * This is what's stored in agent_sessions.agent_config and includes
 * model/provider/temperature that are NOT part of the persistent Assistant entity.
 */
export interface AgentConfig extends Assistant {
  model: string;
  provider: string;
  temperature: number;
  maxTokens?: number;
}

/**
 * Agent session metadata from Rust backend
 */
export interface AgentSession {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  createdAt: Date;
  updatedAt?: Date;
  assistant?: AgentConfig; // Runtime config, not persistent Assistant
}

/**
 * Agent configuration for creating a new session
 */
export interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
}

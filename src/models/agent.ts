import { Assistant } from './chat';

/**
 * Runtime agent configuration (includes session-specific settings)
 * This is what's stored in agent_sessions.agent_config and includes
 * model/provider/temperature that are NOT part of the persistent Assistant entity.
 */

/**
 * Agent session metadata from Rust backend
 */
export interface AgentSession {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  model: string;
  provider: string;
  createdAt: Date;
  updatedAt?: Date;
  assistant?: Assistant; // Runtime config, not persistent Assistant
}

/**
 * Agent configuration for creating a new session
 */
export interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
}

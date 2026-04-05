import { Assistant } from './chat';
import type { SessionAttentionReason } from './agent-ipc';

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
  lastViewedAt?: Date;
  lastMessageAt?: Date;
  lastAttentionAt?: Date;
  lastAttentionReason?: SessionAttentionReason;
  assistant?: Assistant; // Runtime config, not persistent Assistant
  parentSessionId?: string;
  lineageId?: string;
  depth?: number;
  orgId?: string;
  orgName?: string;
  orgRootSessionId?: string;
  isBookmarked?: boolean;
  yoloMode: boolean;
  pendingApprovalCount?: number;
}
/**
 * Agent configuration for creating a new session
 */
export interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
}

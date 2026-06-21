import { Assistant } from './chat';
import type { SessionAttentionReason } from './agent-ipc';
import type { ExecutionMode } from '@/lib/generated/execution-mode';

/**
 * Runtime agent session metadata from Rust backend.
 * Assistant settings are loaded live from the assistants table via `assistantId`.
 */
export interface AgentSession {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error' | 'queued';
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
  executionMode: ExecutionMode;
  pendingApprovalCount?: number;
}
/**
 * Agent configuration for creating a new session
 */
export interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
}

import type { Message, MessageError, RustMessage } from '@/models/chat';
import type { AgentSession } from '@/models/agent';
import type { ParsedStreamChunk } from '@/lib/ai-service/stream-events';
import type {
  AgentRuntimeError,
  PendingApprovalKind,
  PendingApprovalSnapshot,
  PendingUserPromptSummary,
  PreflightTokenMetrics,
  SessionRuntimeState,
  SessionWorkflowStatus,
  UserPromptOutcome,
  UserPromptPayload,
  WorkflowCompletionReason,
} from '@/models/agent-ipc';

export type {
  PendingUserPromptSummary,
  UserPromptPayload,
} from '@/models/agent-ipc';

/** Why a session pane learned its session no longer exists. */
export type SessionGoneReason = 'deleted' | 'not_found';

export type AgentEventPayload =
  | { type: 'workflowStarted'; sessionId: string }
  | {
      type: 'workflowCompleted';
      sessionId: string;
      reason: WorkflowCompletionReason;
      /** User-visible reason when a hook stopped the turn (`reason === 'hookStopped'`). */
      stopReason?: string | null;
    }
  | {
      type: 'hookStoppedNotice';
      sessionId: string;
      /** User-visible reason when a hook stopped a turn while a queued prompt continues the session. */
      stopReason: string;
    }
  | { type: 'workflowError'; sessionId: string; error: AgentRuntimeError }
  | {
      type: 'statusChanged';
      sessionId: string;
      status: SessionWorkflowStatus;
      statusReason?: string | null;
    }
  | { type: 'messageAdded'; sessionId: string; message: RustMessage }
  | { type: 'messagesRemoved'; sessionId: string; messageIds: string[] }
  | { type: 'toolExecutionStarted'; sessionId: string; toolName: string }
  | {
      type: 'toolExecutionCompleted';
      sessionId: string;
      toolName: string;
      success: boolean;
    }
  | {
      type: 'toolExecutionRequiresApproval';
      sessionId: string;
      toolCallId: string;
      toolName: string;
      arguments: string;
      approvalKind: PendingApprovalKind;
      requestId?: string;
      description?: string;
      inputPreview?: string;
    }
  | {
      type: 'channelPermissionRequest';
      sessionId: string;
      requestId: string;
      toolCallId: string;
      toolName: string;
      approvalKind: PendingApprovalKind;
      description: string;
      inputPreview: string;
    }
  | {
      type: 'toolExecutionApprovalResolved';
      sessionId: string;
      toolCallId: string;
      approved: boolean;
    }
  | {
      type: 'sessionRuntimeStateUpdated';
      sessionId: string;
      runtimeState: SessionRuntimeState;
    }
  | {
      type: 'preflightTokenMetricsUpdated';
      sessionId: string;
      metrics: PreflightTokenMetrics;
    }
  | {
      type: 'userPromptRequested';
      sessionId: string;
      requestId: string;
      payload: UserPromptPayload;
    }
  | {
      type: 'userPromptResolved';
      sessionId: string;
      requestId: string;
      outcome: UserPromptOutcome;
    }
  | {
      type: 'resourceUpdated';
      resourceType: string;
      action: string;
      resourceId?: string;
    }
  | {
      type: 'circuitBreakerTriggered';
      sessionId: string;
      toolName: string;
      count: number;
      action:
        | 'hardBreak'
        | 'softRecovery'
        | 'errorEscalate'
        | 'repeatedBatch'
        | 'duplicateInBatch'
        | string;
    }
  | {
      type: 'pendingQueueUpdated';
      sessionId: string;
      messages: RustMessage[];
    }
  | {
      /** The primary model failed; the fallback model is being tried (#2961). */
      type: 'llmFallbackNotice';
      sessionId: string;
      /** HTTP status of the rejected primary request; absent for non-HTTP failures. */
      statusCode?: number;
      fallbackProvider: string;
      fallbackModel: string;
      /** Raw server error, shown in the notice's expandable detail. */
      errorMessage: string;
    }
  | LlmStreamDeltaPayload;

/**
 * One piece of a Rust-routed LLM turn. `delta` has the same shape as a parsed
 * WebView stream chunk, so it feeds `StreamAccumulator` directly.
 */
export interface LlmStreamDeltaPayload {
  type: 'llmStreamDelta';
  sessionId: string;
  responseMessageId: string;
  provider: string;
  /** Output cap the request was sent with; absent when the provider default applies. */
  maxTokens?: number | null;
  delta: ParsedStreamChunk & { finishReason?: string };
}

export type WorkflowPhase =
  | 'idle'
  | 'thinking'
  | 'answering'
  | 'using_tools'
  | 'waiting_approval'
  | 'error';

import type { ExecutionMode } from '@/lib/generated/execution-mode';

export type { ExecutionMode } from '@/lib/generated/execution-mode';
export {
  DEFAULT_EXECUTION_MODE,
  EXECUTION_MODE_VALUES,
  isExecutionMode,
  normalizeExecutionMode,
} from '@/lib/generated/execution-mode';

export type PendingApproval = PendingApprovalSnapshot;

export interface AgentSessionStateContextValue {
  session: AgentSession | null;
  messages: Message[];
  isSessionLoading: boolean;
  isProxyReady: boolean;
  isLoadingOlderMessages: boolean;
  hasOlderMessages: boolean;
  error: MessageError | null;
  llmError: MessageError | null;
  workflowStatus: SessionWorkflowStatus;
  workflowPhase: WorkflowPhase;
  runtimeState: SessionRuntimeState;
  preflightTokenMetrics: PreflightTokenMetrics | null;
  initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null;
  pendingApprovals: PendingApproval[];
  /** A `secret` or `question` prompt waiting for the local user (#2543). */
  pendingUserPrompt: PendingUserPromptSummary | null;
  executionMode: ExecutionMode;
}

export interface AgentSessionActionsContextValue {
  sendMessage: (content: string) => Promise<void>;
  stopSession: () => Promise<void>;
  clearSessionHistory: () => void;
  applyExecutionMode: (mode: ExecutionMode) => void;
  setError: (error: string | AgentRuntimeError | null) => void;
  addMessage: (message: Message) => void;
  loadOlderMessages: () => Promise<void>;
  resumeSession: () => Promise<void>;
  regenerateLastTurn: () => Promise<void>;
  respondToToolApproval: (
    toolCallId: string,
    approved: boolean,
  ) => Promise<void>;
  setExecutionMode: (mode: ExecutionMode) => Promise<void>;
  updateSessionConfig: (model: string, provider: string) => void;
  renameSession: (name: string) => Promise<void>;
}

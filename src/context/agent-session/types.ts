import type { Message, MessageError, RustMessage } from '@/models/chat';
import type { AgentSession } from '@/models/agent';
import type {
  AgentRuntimeError,
  PendingApprovalKind,
  PendingApprovalSnapshot,
  PreflightTokenMetrics,
  SessionRuntimeState,
  WorkflowCompletionReason,
} from '@/models/agent-ipc';

export type AgentEventPayload =
  | { type: 'workflowStarted'; sessionId: string }
  | {
      type: 'workflowCompleted';
      sessionId: string;
      reason: WorkflowCompletionReason;
    }
  | { type: 'workflowError'; sessionId: string; error: AgentRuntimeError }
  | {
      type: 'statusChanged';
      sessionId: string;
      status: 'idle' | 'busy' | 'paused' | 'error';
    }
  | { type: 'messageAdded'; sessionId: string; message: RustMessage }
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
      type: 'interactiveShellInputRequested';
      sessionId: string;
      executionId: string;
      prompt: string;
      inputType: 'password' | 'text';
      command: string;
    }
  | {
      type: 'interactiveShellInputResolved';
      sessionId: string;
      executionId: string;
      outcome: string;
    }
  | {
      type: 'resourceUpdated';
      resourceType: string;
      action: string;
      resourceId?: string;
    };

export interface PendingInteractiveShellPrompt {
  executionId: string;
  prompt: string;
  inputType: 'password' | 'text';
  command: string;
}

export type WorkflowPhase =
  | 'idle'
  | 'thinking'
  | 'answering'
  | 'using_tools'
  | 'waiting_approval'
  | 'error';

export type ExecutionMode = 'normal' | 'yolo' | 'unsafe';

export type PendingApproval = PendingApprovalSnapshot;

export interface AgentSessionStateContextValue {
  session: AgentSession | null;
  messages: Message[];
  isSessionLoading: boolean;
  isLoadingOlderMessages: boolean;
  hasOlderMessages: boolean;
  error: MessageError | null;
  llmError: MessageError | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
  workflowPhase: WorkflowPhase;
  runtimeState: SessionRuntimeState;
  preflightTokenMetrics: PreflightTokenMetrics | null;
  initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null;
  pendingApprovals: PendingApproval[];
  pendingInteractiveShellPrompt: PendingInteractiveShellPrompt | null;
  yoloModeEnabled: boolean;
  unsafeModeEnabled: boolean;
  executionMode: ExecutionMode;
}

export interface AgentSessionActionsContextValue {
  sendMessage: (content: string) => Promise<void>;
  stopSession: () => Promise<void>;
  setError: (error: string | AgentRuntimeError | null) => void;
  addMessage: (message: Message) => void;
  loadOlderMessages: () => Promise<void>;
  resumeSession: () => Promise<void>;
  respondToToolApproval: (
    toolCallId: string,
    approved: boolean,
  ) => Promise<void>;
  setExecutionMode: (mode: ExecutionMode) => Promise<void>;
  toggleYoloMode: () => void;
  toggleUnsafeMode: () => void;
  updateSessionConfig: (model: string, provider: string) => void;
  renameSession: (name: string) => Promise<void>;
}

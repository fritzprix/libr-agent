import type { Message, MessageError, RustMessage } from '@/models/chat';
import type { AgentSession } from '@/models/agent';
import type {
  AgentRuntimeError,
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
    }
  | {
      type: 'channelPermissionRequest';
      sessionId: string;
      requestId: string;
      toolCallId: string;
      toolName: string;
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
      type: 'initializationStep';
      sessionId: string;
      step: string;
      status: 'running' | 'complete' | 'error';
    }
  | {
      type: 'resourceUpdated';
      resourceType: string;
      action: string;
      resourceId?: string;
    };

export type WorkflowPhase =
  | 'idle'
  | 'thinking'
  | 'answering'
  | 'using_tools'
  | 'waiting_approval'
  | 'error';

export interface PendingApproval {
  toolCallId: string;
  toolName: string;
  arguments: string;
}

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
  initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null;
  pendingApprovals: PendingApproval[];
  yoloModeEnabled: boolean;
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
  toggleYoloMode: () => void;
  updateSessionConfig: (model: string, provider: string) => void;
}

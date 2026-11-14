/**
 * Wire types of the shared-backend Session API (docs/api/http_api.md).
 * Only the fields the TUI reads are declared; unknown fields are ignored.
 */

export interface SessionMetadata {
  id: string;
  name?: string | null;
  status: string;
  model: string;
  provider: string;
  assistantId?: string | null;
  parentSessionId?: string | null;
  createdAt: number;
  updatedAt: number;
  lastMessageAt?: number | null;
  executionMode: string;
  workspaceOverride?: string | null;
  workspaceIsolation?: string;
  statusReason?: string | null;
}

export interface PendingApproval {
  toolCallId: string;
  toolName: string;
  reason: string;
  commandPreview: string;
  approvalKind: string;
  requestId?: string;
}

export interface UserQuestion {
  id: string;
  question: string;
  description?: string;
  options?: string[];
  multiSelect: boolean;
  allowCustom: boolean;
}

export interface PendingUserPrompt {
  requestId: string;
  kind: string;
  payload: {
    variableName?: string;
    label?: string;
    reuseNote?: string;
    questions?: UserQuestion[];
  };
  createdAt: string;
}

export interface SessionStatusResponse extends SessionMetadata {
  pendingApprovals: PendingApproval[];
  pendingUserPrompt?: PendingUserPrompt;
}

export interface ListSessionsResponse {
  sessions: SessionMetadata[];
  nextCursor?: { updatedAt: number; id: string } | null;
}

export interface HealthStatus {
  status: string;
  service: string;
  version: string;
  ready: boolean;
  recovery: string;
}

export interface ContentBlock {
  type: string;
  text?: string;
  name?: string;
  input?: unknown;
  content?: unknown;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: string;
  content: ContentBlock[];
  createdAt: number;
  updatedAt: number;
}

export interface GetMessagesResponse {
  messages: ChatMessage[];
}

export interface AssistantInfo {
  id: string;
  name?: string;
  description?: string;
}

export interface CreateSessionBody {
  assistantId: string;
  name?: string;
  model?: string;
  provider?: string;
  workspacePath?: string;
  workspaceIsolation?: string;
  executionMode?: string;
  request?: string;
}

export interface PermissionBody {
  requestId?: string;
  toolCallId?: string;
  behavior: 'allow' | 'deny';
}

export interface PromptBody {
  requestId: string;
  action: 'answer' | 'cancel';
  value?: string;
}

export interface SessionEvent {
  event: string;
  data: unknown;
}

import type { MessageError, RustMessage } from './chat';
import { MCPContent } from '@/lib/mcp';
import type { ExecutionMode } from '@/lib/generated/execution-mode';

/**
 * Agent configuration defining behavior and capabilities.
 * Mirrors `src-tauri/src/agent/config.rs`.
 */
export interface AgentConfig {
  /** Assistant ID (optional, generated if not provided) */
  id?: string;
  /** Alias for id, supported for compatibility */
  assistantId?: string;

  name: string;
  description?: string;
  systemPrompt: string;

  /** MCP server IDs to connect to */
  mcpServerIds: string[];

  /** Local services (legacy) */
  localServices: string[];

  /** Allowed built-in service aliases (undefined = all allowed) */
  allowedBuiltInServiceAliases?: string[];

  maxTokens?: number;

  /** Optional maximum recursive child depth */
  maxDepth?: number;

  /** Optional maximum direct children per parent session */
  maxFanout?: number;

  parentSessionId?: string;
  lineageId?: string;
  depth?: number;
}

export type WorkspaceIsolationMode = 'host' | 'docker';

export interface DockerWorkspaceConfig {
  /** Required for managed Docker sessions; omit when attaching to an existing container. */
  image?: string;
  /** Existing container id/name to attach (Harbor task containers). */
  attachContainer?: string;
  /** Container workdir / file-tool root. Defaults to `/workspace`. */
  workdir?: string;
  /** When false, session cleanup must not stop/remove the container. */
  manageLifecycle?: boolean;
  env?: Record<string, string>;
  portBindings?: DockerPortBinding[];
}

export interface DockerPortBinding {
  containerPort: number;
  hostPort?: number;
}

/**
 * Request payload for creating a new agent session.
 * `agentConfig` is a create-time payload only: backend persists `assistantId` and lineage
 * columns, not a session-level config blob. Assistant settings live in the assistants table.
 * Mirrors `CreateAgentSessionRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface CreateAgentSessionRequest {
  sessionId: string;
  name?: string;
  model?: string;
  provider?: string;
  agentConfig: AgentConfig;
  isEphemeral?: boolean;
  workspacePath?: string;
  workspaceIsolation?: WorkspaceIsolationMode;
  dockerConfig?: DockerWorkspaceConfig;
}

/**
 * Request payload for creating a new session with an initial message.
 * Mirrors `CreateAgentSessionWithMessageRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface CreateAgentSessionWithMessageRequest {
  sessionId: string;
  name?: string;
  model?: string;
  provider?: string;
  agentConfig: AgentConfig;
  message: RustMessage;
  workspacePath?: string;
  workspaceIsolation?: WorkspaceIsolationMode;
  dockerConfig?: DockerWorkspaceConfig;
}

/**
 * Request payload for sending a user message to trigger workflow.
 * Mirrors `SendUserMessageRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface SendUserMessageRequest {
  sessionId: string;
  message: RustMessage;
}

/**
 * Request payload for injecting messages.
 * Workflow continuation is decided by the backend from current session state.
 * `triggerWorkflow` is deprecated and ignored when provided.
 * Mirrors `InjectMessagesRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface InjectMessagesRequest {
  sessionId: string;
  messages: RustMessage[];
  triggerWorkflow?: boolean;
}

/**
 * Request payload for executing a UI-triggered Tauri action through the backend-owned message path.
 * Mirrors `ExecuteUiTauriActionRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface ExecuteUiTauriActionRequest {
  sessionId: string;
  toolName: string;
  params: Record<string, unknown>;
}

/**
 * Request payload for updating session model/provider binding.
 * Mirrors `UpdateAgentConfigRequest` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface UpdateAgentConfigRequest {
  sessionId: string;
  model?: string;
  provider?: string;
  assistantId?: string;
  recursive?: boolean;
}

/**
 * Standard response wrapper for agent operations.
 * Mirrors `AgentResponse` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface AgentResponse<T = unknown> {
  success: boolean;
  message: string;
  data?: T;
}

export type StreamingIssueKind =
  | 'REPEATED_THINKING_LOOP'
  | 'REPEATED_TEXT_LOOP'
  | 'REASONING_BUDGET_EXCEEDED';

export interface StreamingIssueReport {
  sessionId: string;
  responseMessageId: string;
  issueKind: StreamingIssueKind;
  observedTailChars: number;
  /** For reasoning/output-budget: threshold tokens (floor(maxTokens * 0.9)). */
  patternLength: number;
  /** For reasoning/output-budget: estimated non-tool output tokens at abort. */
  repetitionCount: number;
}

export interface CompletionCancelRequest {
  sessionId: string;
  responseMessageId: string;
  reason: string;
}

export type AgentRuntimeError = MessageError;

export type SessionAttentionReason = 'recurringStop' | 'pendingApproval';

export type WorkflowCompletionReason =
  | 'natural'
  | 'recurringStop'
  | 'cancelled'
  | 'terminated';

export type SessionRuntimePhase =
  | 'not_started'
  | 'hydrating'
  | 'initializing'
  | 'ready'
  | 'degraded'
  | 'failed';

export type SessionRuntimeInitResult =
  | 'pending'
  | 'success'
  | 'partial'
  | 'failed';

export type SessionRuntimeProxyMode = 'none' | 'builtin_only' | 'configured';

export type SessionRuntimeTransport = 'stdio' | 'http';

export type SessionRuntimeServerStatus =
  | 'not_started'
  | 'connecting'
  | 'discovering_tools'
  | 'ready'
  | 'failed'
  | 'timed_out';

export interface SessionRuntimeProxyState {
  exists: boolean;
  mode: SessionRuntimeProxyMode;
  ready: boolean;
}

export interface SessionRuntimeDockerState {
  /** Managed image ref, or `attach:<container>`. Absent when unknown. */
  image?: string;
  step?: string;
  progress?: number;
  error?: string;
}

export interface SessionRuntimeInitializationState {
  currentStep?: string;
  result: SessionRuntimeInitResult;
  error?: string;
  docker?: SessionRuntimeDockerState;
}

export interface SessionRuntimeServerState {
  name: string;
  transport: SessionRuntimeTransport;
  status: SessionRuntimeServerStatus;
  toolCount: number;
  error?: string;
}

export interface SessionRuntimeState {
  sequence: number;
  phase: SessionRuntimePhase;
  proxy: SessionRuntimeProxyState;
  initialization: SessionRuntimeInitializationState;
  servers: SessionRuntimeServerState[];
}

export interface PreflightTokenMetrics {
  conservativePromptTokens: number;
  promptAnchoredTotalTokens: number;
  safeInputTokenLimit: number;
  measuredOutputTokensReserve: number;
  effectiveInputBudget: number;
  totalBudgetTokens: number;
  systemPromptTokens: number;
  toolsTokens: number;
  selectedMessageCount: number;
  compactSummaryInjected: boolean;
  preservedCalibrationRatio?: number | null;
}

/**
 * Tool execution result from frontend.
 * Mirrors `ToolExecutionResult` in `src-tauri/src/commands/agent_commands.rs`.
 */
export interface ToolExecutionResult {
  success: boolean;
  content: string;
  mcpContent?: MCPContent[];
  error?: string;
  isError: boolean;
}

/**
 * Session metadata returned by `agent_get_session`.
 * Mirrors `SessionMetadata` in `src-tauri/src/repositories/mod.rs` (implied).
 */
export interface AgentSessionMetadata {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error' | 'queued' | 'provisioning';
  model: string;
  provider: string;
  /** FK to assistants table; authoritative assistant binding for the session */
  assistantId?: string;
  parentSessionId?: string;
  lineageId?: string;
  depth?: number;
  maxDepth?: number;
  maxFanout?: number;
  orgId?: string;
  orgName?: string;
  orgRootSessionId?: string;
  createdAt: number;
  updatedAt?: number;
  lastViewedAt?: number;
  lastMessageAt?: number;
  lastAttentionAt?: number;
  lastAttentionReason?: SessionAttentionReason;
  isBookmarked?: boolean;
  executionMode: ExecutionMode;
  workspaceOverride?: string;
  workspaceIsolation: WorkspaceIsolationMode;
  dockerConfig?: DockerWorkspaceConfig;
  dockerContainerName?: string;
  dockerHostWorkspacePath?: string;
}

export interface AgentSessionListCursor {
  updatedAt: number;
  id: string;
}

export interface AgentSessionListResponse {
  items: AgentSessionMetadata[];
  nextCursor?: AgentSessionListCursor;
}

export interface MessageCursor {
  createdAt: number;
  rowId: number;
}

export interface MessageSlice<TMessage = RustMessage> {
  items: TMessage[];
  hasMoreBefore: boolean;
  oldestCursor?: MessageCursor | null;
}

export type PendingApprovalKind = 'standard' | 'hard';

export interface PendingApprovalSnapshot {
  toolCallId: string;
  toolName: string;
  arguments: string;
  approvalKind: PendingApprovalKind;
  requestId?: string;
  description?: string;
  inputPreview?: string;
}

export interface AgentOpenSessionResponse {
  session: AgentSessionMetadata;
  messages: MessageSlice<RustMessage>;
  pendingApprovals: PendingApprovalSnapshot[];
  runtimeState: SessionRuntimeState;
}

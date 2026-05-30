import type { TokenUsage } from '@/lib/ai-service/types';
import {
  MCPTool,
  MCPContent,
  MCPServerConfig,
  TransportConfig,
  OAuthConfig,
  ServerMetadata,
} from '@/lib/mcp';

// UIResource interface for MCP-UI integration
export interface UIResource {
  uri?: string; // Recommended format: ui://...
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  /**
   * True when this message is a tool result that represents a tool execution failure.
   *
   * UI uses this flag to visually distinguish failed tool results and to group
   * consecutive tool failures into an "error group".
   */
  toolError?: boolean;
  /**
   * Optional normalized category for tool failures (e.g., InvalidInput, NotFound).
   * This is UI-facing and should remain stable even if backend internals change.
   */
  toolErrorCategory?: string;
  text?: string; // inline HTML or remote-dom script
  blob?: string; // base64-encoded content when used
}

export interface AttachmentAgentAccess {
  mode:
    | 'indexed'
    | 'workspace-text'
    | 'workspace-binary'
    | 'inline-media'
    | 'metadata-only';
  reason:
    | 'indexed'
    | 'workspace_only'
    | 'unsupported_extension'
    | 'processing_failed'
    | 'inline_media'
    | 'metadata_only';
  note: string;
}

// MCP file attachment reference type
export interface AttachmentReference {
  sessionId: string; // MCP file store ID (same as session ID)
  contentId?: string; // MCP content ID (undefined for pending/workspace-only files)
  filename: string; // Original filename
  mimeType: string; // MIME type (e.g., 'text/plain', 'text/markdown')
  size: number; // File size (bytes)
  lineCount: number; // Total number of lines
  preview: string; // Preview of the first 10-20 lines
  uploadedAt: string; // Upload time (ISO 8601)
  chunkCount?: number; // Number of chunks (for search purposes)
  lastAccessedAt?: string; // Last access time
  workspacePath?: string; // File path where it's saved in the workspace
  // Explicit state tracking (replaces brittle contentId prefix checking)
  status: 'pending' | 'committed' | 'workspace-only' | 'inline' | 'processing'; // File upload/storage status
  /**
   * AI-facing access guidance for this attachment.
   * This is intentionally separate from lifecycle status so prompts can tell the
   * model which tool family is valid without guessing from contentId/workspacePath.
   */
  agentAccess?: AttachmentAgentAccess;
  pendingId?: string; // Temporary ID for pending files (before commit)
  // For pending files only - used during upload process
  originalUrl?: string; // Original URL or blob URL
  originalPath?: string; // File system path (Tauri environment)
  file?: File; // File object (browser environment)
  blobCleanup?: () => void; // Cleanup function for blob URLs
  /**
   * Populated for image/* and audio/* attachments instead of going through the
   * content store. Holds either a stable file URI or an inline base64 fallback
   * for direct LLM consumption.
   */
  inlineContent?: {
    type: 'image' | 'audio';
    data?: string; // base64-encoded file bytes
    uri?: string; // stable file:// or blob: URL for display and lazy materialization
    mimeType: string;
  };
}

/**
 * Thread represents a logical conversation thread within a session.
 * - Top thread: id === sessionId (always exists)
 * - Sub threads: id !== sessionId (optional, created by user)
 *
 * All threads exist in parallel (no switching concept).
 * Backend manages state via (sessionId, threadId) tuples.
 */
export interface Thread {
  /** Unique thread identifier */
  id: string;

  /** Parent session ID */
  sessionId: string;

  /** Assistant ID for this thread (optional) */
  assistantId?: string;

  /** Initial query or context for this thread (optional) */
  initialQuery?: string;

  /** Thread creation timestamp */
  createdAt: Date;
}

/**
 * Type-safe error type classification for Message errors
 */
export type MessageErrorType =
  | 'MCP_ERROR'
  | 'TOOL_EXECUTION_ERROR'
  | 'AI_SERVICE_ERROR'
  | 'NETWORK_ERROR'
  | 'VALIDATION_ERROR'
  | 'RATE_LIMIT_ERROR'
  | 'MALFORMED_FUNCTION_CALL'
  | 'JSON_PARSING_ERROR'
  | 'AUTHENTICATION_ERROR'
  | 'CONTEXT_LIMIT_ERROR'
  | 'EMPTY_SELECTION_ERROR';

export interface Message {
  id: string;
  sessionId: string; // Added sessionId

  /**
   * Thread ID this message belongs to.
   * REQUIRED: Must always be specified.
   * For top-level thread: threadId === sessionId
   */
  threadId: string;

  role: 'user' | 'assistant' | 'system' | 'tool';
  content: MCPContent[];
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  isStreaming?: boolean;
  /** AI model's internal reasoning process (e.g., chain-of-thought) */
  thinking?: string;
  /** Cryptographic signature or identifier for the thinking content, used for verification or tracking */
  thinkingSignature?: string;
  /** Duration of the thinking process in seconds */
  thinkingTime?: number;
  assistantId?: string; // Optional, used for tracking in multi-agent scenarios
  attachments?: AttachmentReference[]; // Changed to MCP-based file attachment reference
  tool_use?: { id: string; name: string; input: Record<string, unknown> };
  /** Token usage metrics for this message */
  usage?: TokenUsage;
  /** Persisted prompt-token truth for checkpoint-based compaction */
  promptTokens?: number | null;
  createdAt?: Date; // Added
  updatedAt?: Date; // Added
  /** Source of the message - 'assistant' for AI-generated, 'ui' for interface actions, 'channel' for external channel events */
  source?: MessageSource;
  // Error handling for failed AI service calls
  error?: {
    // User-friendly message to display
    displayMessage: string;
    // Error type classification for UI handling
    type: MessageErrorType;
    // Whether the error can be retried
    recoverable: boolean;
    // Detailed logging information (not shown to user)
    details?: {
      originalError: unknown;
      errorCode?: string;
      timestamp: string;
      context?: Record<string, unknown>;
    };
  };
  // Optional metadata for tool execution tracking
  metadata?: {
    executionTime?: number; // Tool execution time in milliseconds
    retryCount?: number; // Number of retry attempts
    [key: string]: unknown; // Extensible for future metadata
  };
}

export type MessageError = NonNullable<Message['error']>;

export const MESSAGE_SOURCES = [
  'assistant',
  'ui',
  'channel',
  'api',
  'tool',
  'compact-summary',
  'compaction-instruction',
  'recovery',
  'session-context',
  'scheduled_task',
] as const;

export type MessageSource = (typeof MESSAGE_SOURCES)[number];

export function isMessageSource(value: unknown): value is MessageSource {
  return (
    typeof value === 'string' &&
    (MESSAGE_SOURCES as readonly string[]).includes(value)
  );
}

export interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

/**
 * RustMessage represents the exact structure of messages coming from the Rust backend.
 * This includes both Tauri command responses and event payloads.
 *
 * Note: Rust uses #[serde(rename_all = "camelCase")] so all fields are in camelCase
 * when serialized to JSON and sent to TypeScript.
 */
export interface RustMessage {
  id: string;
  sessionId: string;
  /**
   * Optional thread ID from backend.
   * If missing, sessionId should be used as the top-level thread ID.
   */
  threadId?: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: MCPContent[];

  // Optional fields - all in camelCase due to serde rename_all
  toolCalls?: ToolCall[];
  toolCallId?: string;
  isStreaming?: boolean;
  thinking?: string;
  thinkingSignature?: string;
  thinkingTime?: number;
  assistantId?: string;
  attachments?: AttachmentReference[];
  toolUse?: { id: string; name: string; input: Record<string, unknown> };
  usage?: TokenUsage;
  promptTokens?: number | null;

  // Timestamps come as Unix milliseconds (i64)
  createdAt: number;
  updatedAt: number;

  source?: MessageSource;
  error?: {
    displayMessage: string;
    type: MessageErrorType;
    recoverable: boolean;
    details?: {
      originalError: unknown;
      errorCode?: string;
      timestamp: string;
      context?: Record<string, unknown>;
    };
  };
  metadata?: {
    executionTime?: number;
    retryCount?: number;
    [key: string]: unknown;
  };
}

/**
 * Convert RustMessage (from Rust backend) to Message (frontend format)
 */
export function rustMessageToMessage(rustMsg: RustMessage): Message {
  return {
    id: rustMsg.id,
    sessionId: rustMsg.sessionId,
    threadId: rustMsg.sessionId, // Fallback to sessionId for threadId
    role: rustMsg.role,
    content: rustMsg.content,
    tool_calls: rustMsg.toolCalls,
    tool_call_id: rustMsg.toolCallId,
    isStreaming: rustMsg.isStreaming,
    thinking: rustMsg.thinking,
    thinkingSignature: rustMsg.thinkingSignature,
    thinkingTime: rustMsg.thinkingTime,
    assistantId: rustMsg.assistantId,
    attachments: rustMsg.attachments,
    tool_use: rustMsg.toolUse,
    usage: rustMsg.usage,
    promptTokens: rustMsg.promptTokens,
    createdAt: new Date(rustMsg.createdAt),
    updatedAt: new Date(rustMsg.updatedAt),
    source: isMessageSource(rustMsg.source) ? rustMsg.source : undefined,
    error: rustMsg.error,
    metadata: rustMsg.metadata,
  };
}

// ========================================
// MCP Configuration Types (MCP 2025-06-18 Spec)
// ========================================

/**
 * Top-level MCP configuration
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfig>;
}

/**
 * MCP Server Entity - Independent server configuration with DB metadata
 * Separates MCP server management from Assistant configuration
 */
export interface MCPServerEntity {
  // Database metadata
  id: string;
  isActive: boolean;
  createdAt: Date;
  updatedAt: Date;

  // MCP Protocol spec (from MCPServerConfig)
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;

  // Cached tool count (from last verification/connection)
  toolCount?: number;
  verificationStatus?: 'pending' | 'success' | 'error';
  lastVerificationError?: string;
}

export interface Assistant {
  id: string;
  name: string;
  description?: string;
  avatar?: string; // Optional avatar URL or identifier
  systemPrompt: string;
  mcpServerIds?: string[]; // References to MCPServerEntity IDs
  localServices?: string[];
  disabledSkills?: string[]; // List of skill names to exclude for this assistant
  /**
   * List of allowed built-in service aliases for this assistant.
   * - Built-in tools follow the format: `builtin_<alias>__<toolname>`
   * - Only tools with aliases in this array will be available to the assistant
   * - `undefined` = all built-in services allowed (default behaviour)
   * - `[]` = no built-in services enabled
   * - Example: ['browser', 'attachments', 'workspace', 'planning', 'playbook']
   */
  allowedBuiltInServiceAliases?: string[];
  deletionProtected: boolean;

  createdAt: Date;
  updatedAt: Date;
}

export interface Tool extends MCPTool {
  isLocal?: boolean;
}

export interface Session {
  id: string;
  type: 'single' | 'group';
  assistants: Assistant[];
  name?: string; // Group name in case of a group session
  description?: string; // Description in case of a group session
  createdAt: Date;
  updatedAt: Date;

  /**
   * Session's top-level thread metadata.
   * This is the only Thread object stored in Session.
   * - id === sessionId (identifies this as the top thread)
   *
   * Other threads exist only in backend state.
   */
  sessionThread: Thread;
}

/**
 * Convert Message (frontend format) to RustMessage (backend format)
 */
export function messageToRustMessage(msg: Message): RustMessage {
  const now = Date.now();

  const createdAt =
    msg.createdAt instanceof Date
      ? msg.createdAt.getTime()
      : typeof msg.createdAt === 'number'
        ? msg.createdAt
        : now;

  const updatedAt =
    msg.updatedAt instanceof Date
      ? msg.updatedAt.getTime()
      : typeof msg.updatedAt === 'number'
        ? msg.updatedAt
        : createdAt;

  return {
    id: msg.id,
    sessionId: msg.sessionId,
    role: msg.role,
    content: msg.content,
    toolCalls: msg.tool_calls
      ? msg.tool_calls.map((tc) => ({
          id: tc.id,
          type: tc.type || 'function',
          function: tc.function,
        }))
      : undefined,
    toolCallId: msg.tool_call_id,
    isStreaming: msg.isStreaming,
    thinking: msg.thinking,
    thinkingSignature: msg.thinkingSignature,
    thinkingTime: msg.thinkingTime,
    assistantId: msg.assistantId,
    attachments: msg.attachments,
    toolUse: msg.tool_use,
    usage: msg.usage,
    promptTokens: msg.promptTokens,
    createdAt,
    updatedAt,
    source: msg.source,
    error: msg.error,
    metadata: msg.metadata,
  };
}

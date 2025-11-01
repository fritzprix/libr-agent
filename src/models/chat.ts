import {
  MCPTool,
  MCPContent,
  MCPServerConfigV2,
  LegacyMCPServerConfig,
  TransportConfig,
  OAuthConfig,
  ServerMetadata,
} from '../lib/mcp-types';

// UIResource interface for MCP-UI integration
export interface UIResource {
  uri?: string; // Recommended format: ui://...
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  text?: string; // inline HTML or remote-dom script
  blob?: string; // base64-encoded content when used
}

// MCP file attachment reference type
export interface AttachmentReference {
  sessionId: string; // MCP file store ID (same as session ID)
  contentId: string; // MCP content ID
  filename: string; // Original filename
  mimeType: string; // MIME type (e.g., 'text/plain', 'text/markdown')
  size: number; // File size (bytes)
  lineCount: number; // Total number of lines
  preview: string; // Preview of the first 10-20 lines
  uploadedAt: string; // Upload time (ISO 8601)
  chunkCount?: number; // Number of chunks (for search purposes)
  lastAccessedAt?: string; // Last access time
  workspacePath?: string; // File path where it's saved in the workspace
  // For pending files only - used during upload process
  originalUrl?: string; // Original URL or blob URL
  originalPath?: string; // File system path (Tauri environment)
  file?: File; // File object (browser environment)
  blobCleanup?: () => void; // Cleanup function for blob URLs
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
  assistantId?: string; // Optional, used for tracking in multi-agent scenarios
  attachments?: AttachmentReference[]; // Changed to MCP-based file attachment reference
  tool_use?: { id: string; name: string; input: Record<string, unknown> };
  createdAt?: Date; // Added
  updatedAt?: Date; // Added
  /** Source of the message - 'assistant' for AI-generated, 'ui' for user interface interactions */
  source?: 'assistant' | 'ui';
  // Error handling for failed AI service calls
  error?: {
    // User-friendly message to display
    displayMessage: string;
    // Error type classification for UI handling
    type: string;
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
}

export interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

// ========================================
// V2 MCP Configuration Types (MCP 2025-06-18 Spec)
// ========================================

/**
 * Top-level MCP configuration supporting both V1 and V2 formats
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfigV2 | LegacyMCPServerConfig>;
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

  // MCP Protocol spec (from MCPServerConfigV2)
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
}

export interface Assistant {
  id?: string;
  name: string;
  description?: string;
  avatar?: string; // Optional avatar URL or identifier
  systemPrompt: string;
  mcpServerIds?: string[]; // References to MCPServerEntity IDs
  localServices?: string[];
  /**
   * List of allowed built-in service aliases for this assistant.
   * - Built-in tools follow the format: `builtin_<alias>__<toolname>`
   * - Only tools with aliases in this array will be available to the assistant
   * - `undefined` = all built-in services allowed (default behaviour)
   * - `[]` = no built-in services enabled
   * - Example: ['browser', 'content_store', 'workspace', 'planning', 'playbook']
   */
  allowedBuiltInServiceAliases?: string[];
  isDefault: boolean;
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

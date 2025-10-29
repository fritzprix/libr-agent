import { MCPTool, MCPContent } from '../lib/mcp-types';

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
 * Discriminated union for transport-specific configurations
 */
export type TransportConfig =
  | {
      type: 'stdio';
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  | {
      type: 'http';
      url: string;
      protocolVersion?: string; // Default: "2025-06-18"
      sessionId?: string;
      headers?: Record<string, string>;
      enableSSE?: boolean; // For backward compatibility with older servers
      security?: {
        enableDnsRebindingProtection?: boolean;
        allowedOrigins?: string[];
        allowedHosts?: string[];
      };
    };

/**
 * OAuth 2.1 authentication configuration (RFC 8414, RFC 7636)
 */
export interface OAuthConfig {
  type: 'oauth2.1';
  discoveryUrl?: string; // RFC 8414 Authorization Server Metadata
  authorizationEndpoint?: string; // Fallback if discovery not available
  tokenEndpoint?: string;
  registrationEndpoint?: string; // RFC 7591 Dynamic Client Registration
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
  usePKCE?: boolean; // Default: true
  resourceParameter?: string; // RFC 9728 Protected Resource Metadata
}

/**
 * Server metadata (optional descriptive information)
 */
export interface ServerMetadata {
  description?: string;
  vendor?: string;
  version?: string;
}

/**
 * V2 MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
 */
export interface MCPServerConfigV2 {
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
}

/**
 * Legacy MCP Server Configuration (stdio-only, for backward compatibility)
 */
export interface LegacyMCPServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

/**
 * Top-level MCP configuration supporting both V1 and V2 formats
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfigV2 | LegacyMCPServerConfig>;
}

/**
 * Type guard to check if config is V2 format
 */
export function isMCPServerConfigV2(
  config: MCPServerConfigV2 | LegacyMCPServerConfig,
): config is MCPServerConfigV2 {
  return 'transport' in config && typeof config.transport === 'object';
}

/**
 * Convert legacy config to V2 format
 */
export function convertLegacyToV2(
  name: string,
  legacy: LegacyMCPServerConfig,
): MCPServerConfigV2 {
  return {
    name,
    transport: {
      type: 'stdio',
      command: legacy.command,
      args: legacy.args,
      env: legacy.env,
    },
  };
}

export interface Assistant {
  id?: string;
  name: string;
  description?: string;
  avatar?: string; // Optional avatar URL or identifier
  systemPrompt: string;
  mcpConfig: MCPConfig;
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

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
  storeId: string; // MCP file store ID
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

export interface Message {
  id: string;
  sessionId: string; // Added sessionId
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

export interface MCPConfig {
  mcpServers?: Record<
    string,
    {
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  >;
}

export interface Assistant {
  id?: string;
  name: string;
  description?: string;
  avatar?: string; // Optional avatar URL or identifier
  systemPrompt: string;
  mcpConfig: MCPConfig;
  localServices?: string[];
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
  storeId?: string; // MCP content-store ID for file attachments
  createdAt: Date;
  updatedAt: Date;
}

export interface Group {
  id: string;
  name: string;
  description: string;
  assistants: Assistant[];
  createdAt: Date;
  updatedAt: Date;
}

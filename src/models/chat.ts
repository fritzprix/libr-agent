import { MCPTool, MCPContent } from '../lib/mcp-types';

// UIResource interface for MCP-UI integration
export interface UIResource {
  uri?: string; // ui://... 형태 권장
  mimeType: string; // 'text/html' | 'text/uri-list' | 'application/vnd.mcp-ui.remote-dom'
  text?: string; // inline HTML or remote-dom script
  blob?: string; // base64-encoded content when used
}

// MCP 파일 첨부 참조 타입
export interface AttachmentReference {
  storeId: string; // MCP 파일 저장소 ID
  contentId: string; // MCP 컨텐츠 ID
  filename: string; // 원본 파일명
  mimeType: string; // MIME 타입 ('text/plain', 'text/markdown' 등)
  size: number; // 파일 크기 (bytes)
  lineCount: number; // 총 라인 수
  preview: string; // 첫 10-20줄 미리보기
  uploadedAt: string; // 업로드 시간 (ISO 8601)
  chunkCount?: number; // 청크 개수 (검색용)
  lastAccessedAt?: string; // 마지막 접근 시간
  workspacePath?: string; // Workspace에 저장된 파일 경로
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
  attachments?: AttachmentReference[]; // MCP 기반 파일 첨부 참조로 변경
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
  name?: string; // Group 세션의 경우 그룹명
  description?: string; // Group 세션의 경우 설명
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

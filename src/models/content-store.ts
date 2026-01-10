import type { RustMCPServerProxy } from '@/hooks/use-rust-mcp-server';

export interface ContentStoreItem {
  sessionId: string;
  contentId: string;
  filename: string;
  mimeType: string;
  size: number;
  lineCount: number;
  preview: string;
  uploadedAt: string;
  chunkCount: number;
  lastAccessedAt?: string;
}

export interface ListContentResult {
  sessionId: string;
  contents: ContentStoreItem[];
  total: number;
  hasMore: boolean;
}

// Backend DTOs
export interface AddContentMetadata {
  filename?: string;
  mimeType?: string;
  size?: number;
  uploadedAt?: string;
}

export interface AddContentArgs {
  fileUrl?: string; // Local file path or blob URL
  srcUrl?: string; // Source URL if web content
  content?: string; // Direct text content
  metadata?: AddContentMetadata;
  title?: string;
  tags?: string[];
}

export interface CreateStoreArgs {
  sessionId?: string;
  metadata?: Record<string, unknown>;
}

export interface ListContentArgs {
  sessionId?: string;
  pagination?: {
    limit?: number;
    offset?: number;
  };
}

export interface DeleteContentArgs {
  contentId: string;
}

export interface ContentStoreServerProxy extends RustMCPServerProxy {
  saveKnowledge(args: AddContentArgs): Promise<ContentStoreItem>;
  listContent(args?: ListContentArgs): Promise<ListContentResult>;
  deleteContent(args: DeleteContentArgs): Promise<void>;
  // Dynamic tool methods from MCP server
  [methodName: string]: unknown;
}

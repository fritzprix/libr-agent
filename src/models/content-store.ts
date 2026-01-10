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

export interface CreateStoreResult {
  sessionId: string;
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

/**
 * Content Store MCP Server Proxy Interface
 *
 * The content store uses a 1:1 relationship between sessions and stores.
 * Each session has exactly one store identified by the sessionId.
 */
export interface ContentStoreServerProxy extends RustMCPServerProxy {
  /**
   * Creates a new content store for the given session.
   * Returns an object containing the sessionId (which IS the storeId in the 1:1 model).
   *
   * @param args - CreateStore arguments including sessionId
   * @returns Promise resolving to CreateStoreResult with sessionId
   */
  createStore(args: CreateStoreArgs): Promise<CreateStoreResult>;
  addContent(args: AddContentArgs): Promise<ContentStoreItem>;
  saveKnowledge(args: AddContentArgs): Promise<ContentStoreItem>;
  listContent(args?: ListContentArgs): Promise<ListContentResult>;
  deleteContent(args: DeleteContentArgs): Promise<void>;
}

// Content Store API Types
// This file defines the TypeScript interfaces for the Content Store MCP server

import { AttachmentReference } from './chat';
import type { MCPTool } from '@/lib/mcp-types';

/**
 * Base interface for Rust MCP server proxies
 */
interface BaseRustMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  [methodName: string]: unknown;
}

/**
 * Metadata for creating a content store
 */
export interface CreateStoreMetadata {
  name?: string;
  description?: string;
}

/**
 * Arguments for creating a content store
 */
export interface CreateStoreArgs {
  sessionId: string;
  metadata?: CreateStoreMetadata;
}

/**
 * Response from creating a content store
 * Note: storeId is deprecated - use sessionId instead (1:1 relationship)
 */
export interface CreateStoreResponse {
  sessionId: string;
  createdAt?: string;
  name?: string;
  description?: string;
}

/**
 * Metadata for adding content
 */
export interface AddContentMetadata {
  filename?: string;
  mimeType?: string;
  size?: number;
  uploadedAt?: string;
}

/**
 * Arguments for adding content to a store
 */
export interface AddContentArgs {
  sessionId: string;
  fileUrl?: string;
  content?: string;
  metadata?: AddContentMetadata;
}

/**
 * Response from adding content
 * Note: Content is stored per session, not per store
 */
export interface AddContentResponse {
  sessionId: string;
  contentId: string;
  filename: string;
  mimeType: string;
  size: number | null;
  lineCount: number;
  preview: string;
  uploadedAt: string;
  chunkCount: number;
}

/**
 * Pagination options for listing content
 */
export interface PaginationOptions {
  offset?: number;
  limit?: number;
}

/**
 * Arguments for listing content
 */
export interface ListContentArgs {
  sessionId: string;
  pagination?: PaginationOptions;
}

/**
 * Content item in list response
 * Note: Content is associated with session, not a separate store
 */
export interface ContentItemSummary {
  sessionId: string;
  contentId: string;
  filename: string;
  mimeType: string;
  size: number | null;
  lineCount?: number;
  preview?: string;
  uploadedAt?: string;
  chunkCount?: number;
  lastAccessedAt?: string;
}

/**
 * Response from listing content
 */
export interface ListContentResponse {
  sessionId: string;
  contents: ContentItemSummary[];
  total: number;
  hasMore: boolean;
}

/**
 * Line range specification
 */
export interface LineRange {
  fromLine: number;
  toLine?: number;
}

/**
 * Arguments for reading content
 * Note: contentId is unique, sessionId is used for validation
 */
export interface ReadContentArgs {
  contentId: string;
  fromLine?: number;
  toLine?: number;
}

/**
 * Response from reading content
 */
export interface ReadContentResponse {
  content: string;
  lineRange: [number, number];
}

/**
 * Search options
 */
export interface SearchOptions {
  topN?: number;
  threshold?: string;
}

/**
 * Search result item
 */
export interface SearchResult {
  contentId: string;
  chunkId: string;
  score: number;
  matchedText: string;
  lineRange: [number, number];
}

/**
 * Arguments for keyword similarity search
 */
export interface KeywordSimilaritySearchArgs {
  sessionId: string;
  query: string;
  options?: SearchOptions;
}

/**
 * Response from keyword similarity search
 */
export interface KeywordSimilaritySearchResponse {
  results: SearchResult[];
}

/**
 * Arguments for deleting content
 */
export interface DeleteContentArgs {
  contentId: string;
}

/**
 * Response from deleting content
 */
export interface DeleteContentResponse {
  contentId: string;
  sessionId: string;
}

/**
 * Content Store Server Proxy Interface
 * Defines the methods available on the content store MCP server
 */
export interface ContentStoreServerProxy extends BaseRustMCPServerProxy {
  createStore: (args: CreateStoreArgs) => Promise<CreateStoreResponse>;
  addContent: (args: AddContentArgs) => Promise<AddContentResponse>;
  listContent: (args: ListContentArgs) => Promise<ListContentResponse>;
  readContent: (args: ReadContentArgs) => Promise<ReadContentResponse>;
  keywordSimilaritySearch: (
    args: KeywordSimilaritySearchArgs,
  ) => Promise<KeywordSimilaritySearchResponse>;
  deleteContent: (args: DeleteContentArgs) => Promise<DeleteContentResponse>;
}

/**
 * File input for adding to pending files
 */
export interface PendingFileInput {
  url: string;
  mimeType: string;
  filename?: string;
  originalPath?: string; // File system path (Tauri environment)
  file?: File; // File object (browser environment)
  blobCleanup?: () => void; // Cleanup function for blob URLs
}

/**
 * Extended AttachmentReference with additional fields for pending files
 */
export interface ExtendedAttachmentReference extends AttachmentReference {
  // Additional fields for pending file handling
  originalUrl?: string;
  originalPath?: string;
  file?: File;
  blobCleanup?: () => void;
}

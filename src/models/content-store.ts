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
  sessionId?: string;
  name?: string;
  description?: string;
}

/**
 * Arguments for creating a content store
 */
export interface CreateStoreArgs {
  metadata?: CreateStoreMetadata;
}

/**
 * Response from creating a content store
 */
export interface CreateStoreResponse {
  storeId?: string;
  id?: string;
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
  storeId: string;
  fileUrl?: string;
  content?: string;
  metadata?: AddContentMetadata;
}

/**
 * Response from adding content
 */
export interface AddContentResponse {
  storeId: string;
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
  storeId: string;
  pagination?: PaginationOptions;
}

/**
 * Content item in list response
 */
export interface ContentItemSummary {
  storeId: string;
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
  storeId: string;
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
 */
export interface ReadContentArgs {
  storeId: string;
  contentId: string;
  lineRange: LineRange;
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
 * Arguments for keyword similarity search
 */
export interface KeywordSimilaritySearchArgs {
  storeId: string;
  query: string;
  options?: SearchOptions;
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
 * Response from keyword similarity search
 */
export interface KeywordSimilaritySearchResponse {
  results: SearchResult[];
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

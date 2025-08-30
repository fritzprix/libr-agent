/**
 * Content Store Module Types
 *
 * Shared type definitions for the content store module
 */

import type { SearchResult } from '@/models/search-engine';

// File size limits (in bytes)
export const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50MB
export const MAX_CONTENT_LENGTH = 10 * 1024 * 1024; // 10MB text content

// Custom error classes for better error handling
export class FileStoreError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly context?: Record<string, unknown>,
  ) {
    super(message);
    this.name = 'FileStoreError';
  }
}

export class StoreNotFoundError extends FileStoreError {
  constructor(storeId: string) {
    super(`Store not found: ${storeId}`, 'STORE_NOT_FOUND', { storeId });
  }
}

export class ContentNotFoundError extends FileStoreError {
  constructor(contentId: string, storeId?: string) {
    super(`Content not found: ${contentId}`, 'CONTENT_NOT_FOUND', {
      contentId,
      storeId,
    });
  }
}

export class InvalidRangeError extends FileStoreError {
  constructor(fromLine: number, toLine: number, totalLines: number) {
    super(
      `Invalid line range: ${fromLine}-${toLine} (total: ${totalLines})`,
      'INVALID_LINE_RANGE',
      { fromLine, toLine, totalLines },
    );
  }
}

export interface ParseResult {
  content: string;
  filename: string;
  mimeType: string;
  size: number;
}

// Common interfaces used across the module
export interface StoreInfo {
  id: string;
  name: string;
  description?: string;
  contentCount: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface ContentInfo {
  id: string;
  storeId: string;
  filename: string;
  mimeType: string;
  size: number;
  chunkCount: number;
  contentHash: string;
  createdAt: Date;
  updatedAt: Date;
}

// Search related types
export interface SearchOptions {
  limit?: number;
  scoreThreshold?: number;
  includeContent?: boolean;
}

export interface ContentSearchResult extends SearchResult {
  contentId: string;
  storeId: string;
  filename: string;
  mimeType: string;
  chunk?: {
    id: string;
    position: number;
    content: string;
  };
}

// Tool parameter interfaces for type safety
export interface CreateStoreParams {
  name: string;
  description?: string;
}

export interface ListStoresParams {
  includeStats?: boolean;
}

export interface AddFileParams {
  storeId: string;
  file: File;
  filename?: string;
  overwrite?: boolean;
}

export interface SearchParams {
  storeId: string;
  query: string;
  limit?: number;
  scoreThreshold?: number;
  includeContent?: boolean;
}

export interface ReadContentParams {
  storeId: string;
  contentId: string;
  fromLine?: number;
  toLine?: number;
}

export interface DeleteContentParams {
  storeId: string;
  contentId: string;
}

export interface DeleteStoreParams {
  storeId: string;
}

export interface ListContentParams {
  storeId: string;
  limit?: number;
  offset?: number;
}

/**
 * Type definitions for Rust backend client
 */

import type { ServiceMetadata } from '@/features/mcp/types';

/**
 * Represents an item in the workspace file system.
 */
export interface WorkspaceFileItem {
  /** The name of the file or directory. */
  name: string;
  /** True if the item is a directory. */
  isDirectory: boolean;
  /** The full path to the item. */
  path: string;
  /** The size of the file in bytes, or null for directories. */
  size?: number | null;
  /** The last modified timestamp as an ISO string, or null. */
  modified?: string | null;
}

/**
 * Represents an active browser session controlled by the backend.
 */
export interface BrowserSession {
  /** The unique identifier for the session. */
  id: string;
  /** The current URL of the browser session. */
  url: string;
  /** The title of the current page. */
  title?: string | null;
}

/**
 * Parameters for creating a new browser session.
 */
export type BrowserSessionParams = {
  /** The initial URL to navigate to. */
  url: string;
  /** An optional title for the session. */
  title?: string | null;
};

/**
 * The result of a script execution in the browser.
 * It can be a string for a successful result, null for no result, or an object with an error message.
 */
export type ScriptResult = string | null | { error?: string };

/**
 * Search result for message queries.
 */
export interface MessageSearchResult {
  /** The unique ID of the message */
  messageId: string;
  /** The session ID this message belongs to */
  sessionId: string;
  /** BM25 relevance score */
  score: number;
  /** Optional text snippet containing the search query */
  snippet?: string;
  /** Message creation timestamp */
  createdAt: Date;
}

/**
 * Complete information about a builtin server including metadata.
 */
export interface BuiltinServerInfo {
  /** Server identifier (e.g., "workspace", "contentstore") */
  name: string;
  /** UI metadata (displayName, description, category, icon) */
  metadata: ServiceMetadata;
  /** Number of tools this server provides */
  toolCount: number;
}

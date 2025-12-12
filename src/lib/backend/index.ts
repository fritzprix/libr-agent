/**
 * 🔌 Rust Backend Client
 *
 * Unified client for all Tauri backend communication.
 * Provides centralized error handling, logging, and consistent API.
 * Used by both React hooks and non-React services.
 *
 * This is the main entry point that re-exports all backend client modules.
 */

// Core utilities
export { safeInvoke } from './core';

// Type exports
export type {
  WorkspaceFileItem,
  BrowserSession,
  BrowserSessionParams,
  ScriptResult,
  MessageSearchResult,
  BuiltinServerInfo,
} from './types';

// Workspace management
export { listWorkspaceFiles, workspaceWriteFile } from './workspace';

// MCP server management
export {
  startServer,
  stopServer,
  callTool,
  listTools,
  listToolsFromConfig,
  startOAuthFlow,
  completeOAuthFlow,
  hasOAuthToken,
  getOAuthToken,
  revokeOAuthToken,
  getConnectedServers,
  checkServerStatus,
  checkAllServersStatus,
  sampleFromModel,
  listAllTools,
  getValidatedTools,
  validateToolSchema,
} from './mcp-server';

// Built-in tools
export {
  listBuiltinServers,
  listBuiltinTools,
  listBuiltinServersWithMetadata,
  callBuiltinTool,
  listAllToolsUnified,
  callToolUnified,
} from './builtin-tools';

// Browser operations
export {
  createBrowserSession,
  closeBrowserSession,
  listBrowserSessions,
  clickElement,
  inputText,
  pollScriptResult,
  navigateToUrl,
  navigateBack,
  navigateForward,
} from './browser';

// File operations
export { readFile, readDroppedFile, writeFile } from './file-operations';

// Message management
export {
  getMessagesPageForSession,
  upsertMessages,
  upsertMessage,
  deleteMessage,
  deleteAllMessagesForSession,
  searchMessages,
} from './messages';

// Session management
export { removeSession, deleteContentStore, switchSession } from './sessions';

// Utilities
export {
  getAppLogsDir,
  backupCurrentLog,
  clearCurrentLog,
  listLogFiles,
  openExternalUrl,
  downloadWorkspaceFile,
  exportAndDownloadZip,
  getServiceContext,
  switchContext,
  greet,
} from './utils';

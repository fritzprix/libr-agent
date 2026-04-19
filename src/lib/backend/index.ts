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
export {
  listWorkspaceFiles,
  workspaceWriteFile,
  openWorkspaceFileWithDefaultApp,
  openWorkspaceInExplorer,
  openWorkspaceInTerminal,
  getWorkspaceOverride,
  setWorkspaceOverride,
  cancelWorkspaceOverride,
  getWorkspaceDir,
} from './workspace';

// MCP server management
export {
  callTool,
  hasOAuthToken,
  getOAuthToken,
  revokeOAuthToken,
  sampleFromModel,
  validateToolSchema,
} from './mcp-server';

// Built-in tools
export {
  listBuiltinServers,
  listBuiltinTools,
  listBuiltinServersWithMetadata,
  listAvailableBuiltinServerDefinitions,
} from './builtin-tools';

// Browser operations
export {
  createBrowserSession,
  closeBrowserSession,
  listBrowserSessions,
  navigateToUrl,
} from './browser';

// File operations
export {
  checkDroppedPathType,
  readDroppedFile,
  registerDroppedFiles,
  writeFile,
} from './file-operations';

// Message management
export {
  getMessagesPageForSession,
  upsertMessages,
  upsertMessage,
  deleteMessage,
  deleteAllMessagesForSession,
  searchMessages,
} from './messages';

// Agent Workflow & Commands
export {
  handleLLMResponse,
  handleUserToolCall,
  executeUiTauriAction,
  getAgentAvailableTools,
  agentCallBuiltinTool,
} from './agent-commands';

// Session management
export { removeSession, deleteAttachments } from './sessions';

// Knowledge management
export {
  listGlobalKnowledge,
  getGlobalKnowledgeDetail,
  deleteGlobalKnowledge,
} from './knowledge';

// Utilities
export {
  getAppLogsDir,
  backupCurrentLog,
  clearCurrentLog,
  listLogFiles,
  openExternalUrl,
  downloadMediaFile,
  downloadWorkspaceFile,
  exportAndDownloadZip,
  getServiceContext,
  greet,
  restartApp,
} from './utils';

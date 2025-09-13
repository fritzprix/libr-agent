import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import {
  MCPServerConfig,
  MCPTool,
  MCPResponse,
  SamplingOptions,
  SamplingResponse,
} from './mcp-types';

const logger = getLogger('RustBackendClient');

// ========================================
// Workspace types / interfaces
// ========================================

export interface WorkspaceFileItem {
  name: string;
  isDirectory: boolean;
  path: string;
  size?: number | null;
  modified?: string | null;
}

// ========================================
// Browser types / interfaces
// ========================================

export interface BrowserSession {
  id: string;
  url: string;
  title?: string | null;
}

export type BrowserSessionParams = {
  url: string;
  title?: string | null;
};

export type ScriptResult = string | null | { error?: string };

/**
 * 🔌 Shared Rust Backend Client
 *
 * Unified client for all Tauri backend communication.
 * Provides centralized error handling, logging, and consistent API.
 * Used by both React hooks and non-React services.
 */

async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    logger.debug('invoke', { cmd, args });
    return await invoke<T>(cmd, args ?? {});
  } catch (err) {
    logger.error('invoke failed', { cmd, err });
    throw err;
  }
}

// ========================================
// Workspace Management
// ========================================

export async function listWorkspaceFiles(
  path?: string,
): Promise<WorkspaceFileItem[]> {
  return safeInvoke<WorkspaceFileItem[]>(
    'list_workspace_files',
    path ? { path } : {},
  );
}

// ========================================
// MCP Server Management
// ========================================

export async function startServer(config: MCPServerConfig): Promise<string> {
  return safeInvoke<string>('start_mcp_server', { config });
}

export async function stopServer(serverName: string): Promise<void> {
  return safeInvoke<void>('stop_mcp_server', { serverName });
}

export async function callTool(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_mcp_tool', {
    serverName,
    toolName,
    arguments: args,
  });
}

export async function listTools(serverName: string): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_mcp_tools', { serverName });
}

export async function listToolsFromConfig(config: {
  mcpServers?: Record<
    string,
    { command: string; args?: string[]; env?: Record<string, string> }
  >;
}): Promise<Record<string, MCPTool[]>> {
  return safeInvoke<Record<string, MCPTool[]>>('list_tools_from_config', {
    config,
  });
}

export async function getConnectedServers(): Promise<string[]> {
  return safeInvoke<string[]>('get_connected_servers');
}

export async function checkServerStatus(serverName: string): Promise<boolean> {
  return safeInvoke<boolean>('check_server_status', { serverName });
}

export async function checkAllServersStatus(): Promise<
  Record<string, boolean>
> {
  return safeInvoke<Record<string, boolean>>('check_all_servers_status');
}

export async function sampleFromModel(
  serverName: string,
  prompt: string,
  options?: SamplingOptions,
): Promise<SamplingResponse> {
  return safeInvoke<SamplingResponse>('sample_from_mcp_server', {
    serverName,
    prompt,
    options,
  });
}

// ========================================
// Built-in Tools
// ========================================

export async function listBuiltinServers(): Promise<string[]> {
  return safeInvoke<string[]>('list_builtin_servers');
}

export async function listBuiltinTools(
  serverName?: string,
): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>(
    'list_builtin_tools',
    serverName ? { serverName } : undefined,
  );
}

export async function callBuiltinTool(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_builtin_tool', {
    serverName,
    toolName,
    arguments: args,
  });
}

// ========================================
// Unified Tools API
// ========================================

export async function listAllToolsUnified(): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_all_tools_unified');
}

export async function callToolUnified(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_tool_unified', {
    serverName,
    toolName,
    arguments: args,
  });
}

// ========================================
// Validation Tools
// ========================================

export async function listAllTools(): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_all_tools');
}

export async function getValidatedTools(
  serverName: string,
): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('get_validated_tools', { serverName });
}

export async function validateToolSchema(tool: MCPTool): Promise<void> {
  return safeInvoke<void>('validate_tool_schema', { tool });
}

// ========================================
// File System Operations
// ========================================

export async function readFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_file', { filePath });
}

export async function readDroppedFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_dropped_file', { filePath });
}

export async function writeFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('write_file', { filePath, content });
}

// ========================================
// Browser session / script helpers
// Centralized wrappers for browser-related Tauri commands used by
// `BrowserToolProvider` and other browser features. These use `safeInvoke`
// so logging and error handling remain consistent across the app.
// ========================================

export async function createBrowserSession(params: {
  url: string;
  title?: string | null;
}): Promise<string> {
  return safeInvoke<string>('create_browser_session', params);
}

export async function closeBrowserSession(sessionId: string): Promise<void> {
  return safeInvoke<void>('close_browser_session', { sessionId });
}

export async function listBrowserSessions(): Promise<BrowserSession[]> {
  return safeInvoke<BrowserSession[]>('list_browser_sessions');
}

export async function clickElement(
  sessionId: string,
  selector: string,
): Promise<string> {
  return safeInvoke<string>('click_element', { sessionId, selector });
}

export async function inputText(
  sessionId: string,
  selector: string,
  text: string,
): Promise<string> {
  return safeInvoke<string>('input_text', { sessionId, selector, text });
}

export async function pollScriptResult(
  requestId: string,
): Promise<string | null> {
  return safeInvoke<string | null>('poll_script_result', { requestId });
}

export async function navigateToUrl(
  sessionId: string,
  url: string,
): Promise<string> {
  return safeInvoke<string>('navigate_to_url', { sessionId, url });
}

// ========================================
// Log Management
// ========================================

export async function getAppLogsDir(): Promise<string> {
  return safeInvoke<string>('get_app_logs_dir');
}

export async function backupCurrentLog(): Promise<string> {
  return safeInvoke<string>('backup_current_log');
}

export async function clearCurrentLog(): Promise<void> {
  return safeInvoke<void>('clear_current_log');
}

export async function listLogFiles(): Promise<string[]> {
  return safeInvoke<string[]>('list_log_files');
}

// ========================================
// External URL handling
// ========================================

export async function openExternalUrl(url: string): Promise<void> {
  return safeInvoke<void>('open_external_url', { url });
}

// ========================================
// File Download Operations
// ========================================

export async function downloadWorkspaceFile(filePath: string): Promise<string> {
  return safeInvoke<string>('download_workspace_file', { filePath });
}

export async function exportAndDownloadZip(
  files: string[],
  packageName: string,
): Promise<string> {
  return safeInvoke<string>('export_and_download_zip', { files, packageName });
}

// ========================================
// Utility
// ========================================

export async function greet(name: string): Promise<string> {
  return safeInvoke<string>('greet', { name });
}

// Export default object for compatibility
export default {
  safeInvoke,
  startServer,
  stopServer,
  callTool,
  listTools,
  listToolsFromConfig,
  getConnectedServers,
  checkServerStatus,
  checkAllServersStatus,
  sampleFromModel,
  listBuiltinServers,
  listBuiltinTools,
  callBuiltinTool,
  listAllToolsUnified,
  callToolUnified,
  listAllTools,
  getValidatedTools,
  validateToolSchema,
  readFile,
  readDroppedFile,
  writeFile,
  getAppLogsDir,
  backupCurrentLog,
  listWorkspaceFiles,
  clearCurrentLog,
  listLogFiles,
  openExternalUrl,
  downloadWorkspaceFile,
  exportAndDownloadZip,
  greet,
};

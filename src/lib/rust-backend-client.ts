import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import {
  MCPServerConfig,
  MCPTool,
  MCPResponse,
  SamplingOptions,
  SamplingResponse,
} from './mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';

const logger = getLogger('RustBackendClient');

// ========================================
// Workspace types / interfaces
// ========================================

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

// ========================================
// Browser types / interfaces
// ========================================

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
 * 🔌 Shared Rust Backend Client
 *
 * Unified client for all Tauri backend communication.
 * Provides centralized error handling, logging, and consistent API.
 * Used by both React hooks and non-React services.
 */

/**
 * A wrapper around Tauri's `invoke` function that provides centralized
 * logging and error handling for all backend calls.
 *
 * @template T The expected return type of the invoked command.
 * @param cmd The name of the command to invoke on the backend.
 * @param args Optional arguments for the command.
 * @returns A promise that resolves with the result of the command.
 * @throws Rethrows the error from the backend if the invocation fails.
 * @internal
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

/**
 * Lists the files and directories in the specified workspace path.
 * @param path The optional path within the workspace to list. Defaults to the root.
 * @returns A promise that resolves to an array of `WorkspaceFileItem` objects.
 */
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

/**
 * Starts an MCP server on the backend.
 * @param config The configuration for the server to start.
 * @returns A promise that resolves with a message from the backend.
 */
export async function startServer(config: MCPServerConfig): Promise<string> {
  return safeInvoke<string>('start_mcp_server', { config });
}

/**
 * Stops a running MCP server.
 * @param serverName The name of the server to stop.
 * @returns A promise that resolves when the server has been stopped.
 */
export async function stopServer(serverName: string): Promise<void> {
  return safeInvoke<void>('stop_mcp_server', { serverName });
}

/**
 * Calls a tool on a specified MCP server.
 * @param serverName The name of the server.
 * @param toolName The name of the tool to call.
 * @param args The arguments to pass to the tool.
 * @returns A promise that resolves to an `MCPResponse`.
 */
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

/**
 * Lists the tools available on a specified MCP server.
 * @param serverName The name of the server.
 * @returns A promise that resolves to an array of `MCPTool` objects.
 */
export async function listTools(serverName: string): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_mcp_tools', { serverName });
}

/**
 * Lists tools from a given configuration object without starting the servers.
 * @param config The configuration object containing MCP server definitions.
 * @returns A promise that resolves to a record mapping server names to their tool lists.
 */
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

/**
 * Gets a list of all currently connected MCP servers.
 * @returns A promise that resolves to an array of connected server names.
 */
export async function getConnectedServers(): Promise<string[]> {
  return safeInvoke<string[]>('get_connected_servers');
}

/**
 * Checks the status of a specific MCP server.
 * @param serverName The name of the server to check.
 * @returns A promise that resolves to true if the server is running, false otherwise.
 */
export async function checkServerStatus(serverName: string): Promise<boolean> {
  return safeInvoke<boolean>('check_server_status', { serverName });
}

/**
 * Checks the status of all configured MCP servers.
 * @returns A promise that resolves to a record mapping server names to their running status.
 */
export async function checkAllServersStatus(): Promise<
  Record<string, boolean>
> {
  return safeInvoke<Record<string, boolean>>('check_all_servers_status');
}

/**
 * Performs text generation (sampling) using a model on a specified MCP server.
 * @param serverName The name of the server.
 * @param prompt The prompt to send to the model.
 * @param options Optional sampling parameters.
 * @returns A promise that resolves to a `SamplingResponse`.
 */
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

/**
 * Lists the names of all available built-in servers.
 * @returns A promise that resolves to an array of server names.
 */
export async function listBuiltinServers(): Promise<string[]> {
  return safeInvoke<string[]>('list_builtin_servers');
}

/**
 * Lists the tools provided by a built-in server.
 * @param serverName The optional name of the server. If not provided, lists tools for all built-in servers.
 * @returns A promise that resolves to an array of `MCPTool` objects.
 */
export async function listBuiltinTools(
  serverName?: string,
): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>(
    'list_builtin_tools',
    serverName ? { serverName } : undefined,
  );
}

/**
 * Calls a tool on a built-in server.
 * @param serverName The name of the built-in server.
 * @param toolName The name of the tool to call.
 * @param args The arguments to pass to the tool.
 * @returns A promise that resolves to an `MCPResponse`.
 */
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

/**
 * Lists all tools from all available sources (MCP servers, built-in, etc.)
 * in a unified list.
 * @returns A promise that resolves to a single array of all `MCPTool` objects.
 */
export async function listAllToolsUnified(): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_all_tools_unified');
}

/**
 * Calls a tool from any available source using a unified interface.
 * The backend will resolve the correct server and tool to call.
 * @param serverName The name of the server providing the tool.
 * @param toolName The name of the tool to call.
 * @param args The arguments to pass to the tool.
 * @returns A promise that resolves to an `MCPResponse`.
 */
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

/**
 * Lists all tools from all sources, including those that may not be valid.
 * @returns A promise that resolves to an array of all discovered `MCPTool` objects.
 */
export async function listAllTools(): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_all_tools');
}

/**
 * Gets a list of tools from a server that have been successfully validated.
 * @param serverName The name of the server.
 * @returns A promise that resolves to an array of validated `MCPTool` objects.
 */
export async function getValidatedTools(
  serverName: string,
): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('get_validated_tools', { serverName });
}

/**
 * Validates the schema of a single tool.
 * @param tool The `MCPTool` object to validate.
 * @returns A promise that resolves if the schema is valid, or rejects otherwise.
 */
export async function validateToolSchema(tool: MCPTool): Promise<void> {
  return safeInvoke<void>('validate_tool_schema', { tool });
}

// ========================================
// File System Operations
// ========================================

/**
 * Reads the content of a file from the filesystem.
 * @param filePath The path to the file to read.
 * @returns A promise that resolves to an array of numbers representing the file's byte content.
 */
export async function readFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_file', { filePath });
}

/**
 * Reads the content of a file that was dropped onto the application window.
 * @param filePath The path of the dropped file.
 * @returns A promise that resolves to an array of numbers representing the file's byte content.
 */
export async function readDroppedFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_dropped_file', { filePath });
}

/**
 * Writes content to a file in the filesystem.
 * @param filePath The path to the file to write to.
 * @param content An array of numbers representing the byte content to write.
 * @returns A promise that resolves when the write operation is complete.
 */
export async function writeFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('write_file', { filePath, content });
}

/**
 * Writes content to a file within the application's workspace directory.
 * @param filePath The relative path within the workspace to write to.
 * @param content An array of numbers representing the byte content to write.
 * @returns A promise that resolves when the write operation is complete.
 */
export async function workspaceWriteFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('workspace_write_file', {
    filePath,
    content,
  });
}

// ========================================
// Browser Session and Scripting Helpers
// Centralized wrappers for browser-related Tauri commands used by
// `BrowserToolProvider` and other browser features. These use `safeInvoke`
// so logging and error handling remain consistent across the app.
// ========================================

/**
 * Creates a new browser session controlled by the backend.
 * @param params The parameters for the new session, including the initial URL.
 * @returns A promise that resolves to the unique ID of the new session.
 */
export async function createBrowserSession(params: {
  url: string;
  title?: string | null;
}): Promise<string> {
  return safeInvoke<string>('create_browser_session', params);
}

/**
 * Closes an active browser session.
 * @param sessionId The ID of the session to close.
 * @returns A promise that resolves when the session is closed.
 */
export async function closeBrowserSession(sessionId: string): Promise<void> {
  return safeInvoke<void>('close_browser_session', { sessionId });
}

/**
 * Lists all active browser sessions.
 * @returns A promise that resolves to an array of `BrowserSession` objects.
 */
export async function listBrowserSessions(): Promise<BrowserSession[]> {
  return safeInvoke<BrowserSession[]>('list_browser_sessions');
}

/**
 * Simulates a click on an element in a browser session.
 * @param sessionId The ID of the browser session.
 * @param selector The CSS selector of the element to click.
 * @returns A promise that resolves with the result of the script execution.
 */
export async function clickElement(
  sessionId: string,
  selector: string,
): Promise<string> {
  return safeInvoke<string>('click_element', { sessionId, selector });
}

/**
 * Inputs text into an element in a browser session.
 * @param sessionId The ID of the browser session.
 * @param selector The CSS selector of the input element.
 * @param text The text to input.
 * @returns A promise that resolves with the result of the script execution.
 */
export async function inputText(
  sessionId: string,
  selector: string,
  text: string,
): Promise<string> {
  return safeInvoke<string>('input_text', { sessionId, selector, text });
}

/**
 * Polls for the result of a previously executed asynchronous script.
 * @param requestId The ID of the script execution request to poll.
 * @returns A promise that resolves to the script result, or null if it's not ready.
 */
export async function pollScriptResult(
  requestId: string,
): Promise<string | null> {
  return safeInvoke<string | null>('poll_script_result', { requestId });
}

/**
 * Navigates a browser session to a new URL.
 * @param sessionId The ID of the browser session.
 * @param url The URL to navigate to.
 * @returns A promise that resolves with the result of the navigation.
 */
export async function navigateToUrl(
  sessionId: string,
  url: string,
): Promise<string> {
  return safeInvoke<string>('navigate_to_url', { sessionId, url });
}

// ========================================
// Log Management
// ========================================

/**
 * Gets the directory where application logs are stored.
 * @returns A promise that resolves to the absolute path of the log directory.
 */
export async function getAppLogsDir(): Promise<string> {
  return safeInvoke<string>('get_app_logs_dir');
}

/**
 * Creates a backup of the current log file.
 * @returns A promise that resolves to the path of the newly created backup file.
 */
export async function backupCurrentLog(): Promise<string> {
  return safeInvoke<string>('backup_current_log');
}

/**
 * Clears the content of the current log file.
 * @returns A promise that resolves when the log file has been cleared.
 */
export async function clearCurrentLog(): Promise<void> {
  return safeInvoke<void>('clear_current_log');
}

/**
 * Lists all log files in the application's log directory.
 * @returns A promise that resolves to an array of log file names.
 */
export async function listLogFiles(): Promise<string[]> {
  return safeInvoke<string[]>('list_log_files');
}

// ========================================
// External URL Handling
// ========================================

/**
 * Opens a URL in the user's default external browser.
 * @param url The URL to open.
 * @returns A promise that resolves when the URL has been opened.
 */
export async function openExternalUrl(url: string): Promise<void> {
  return safeInvoke<void>('open_external_url', { url });
}

// ========================================
// File Download Operations
// ========================================

/**
 * Initiates a download of a file from the workspace.
 * @param filePath The path of the file within the workspace to download.
 * @returns A promise that resolves to a string indicating the download status or path.
 */
export async function downloadWorkspaceFile(filePath: string): Promise<string> {
  return safeInvoke<string>('download_workspace_file', { filePath });
}

/**
 * Exports a selection of files as a zip archive and initiates a download.
 * @param files An array of file paths to include in the zip archive.
 * @param packageName The name for the zip package.
 * @returns A promise that resolves to a string indicating the download status or path.
 */
export async function exportAndDownloadZip(
  files: string[],
  packageName: string,
): Promise<string> {
  return safeInvoke<string>('export_and_download_zip', { files, packageName });
}

// ========================================
// Utility
// ========================================

/**
 * Gets the service context for a given server.
 * @param serverId The ID of the server.
 * @returns A promise that resolves to the service context.
 */
export async function getServiceContext(
  serverId: string,
): Promise<ServiceContext<unknown>> {
  return safeInvoke<ServiceContext<unknown>>('get_service_context', {
    serverId,
  });
}

/**
 * Switches the context for a given server.
 * @param serverId The ID of the server.
 * @param options The context options to switch to.
 * @returns A promise that resolves when the context is switched.
 */
export async function switchContext(
  serverId: string,
  options: ServiceContextOptions,
): Promise<void> {
  return safeInvoke<void>('switch_context', { serverId, options });
}

/**
 * A simple utility function to test the backend connection.
 * @param name A name to include in the greeting.
 * @returns A promise that resolves to a greeting string from the backend.
 */
export async function greet(name: string): Promise<string> {
  return safeInvoke<string>('greet', { name });
}

/**
 * Remove a session including its workspace directory on the native side.
 * This calls the Tauri command `remove_session` implemented in the backend.
 * @param sessionId The ID of the session to remove
 */
export async function removeSession(sessionId: string): Promise<void> {
  return safeInvoke<void>('remove_session', { sessionId });
}

/**
 * Delete content store artifacts for a session (backend command).
 * This removes SQLite rows and search index directories for the given session.
 */
export async function deleteContentStore(sessionId: string): Promise<void> {
  return safeInvoke<void>('delete_content_store', { sessionId });
}

/**
 * Switches to a specific session with optional async behavior.
 * @param sessionId The ID of the session to switch to
 * @param useAsync Whether to use async switching (default: true)
 * @returns A promise that resolves with session information
 */
export async function switchSession(
  sessionId: string,
  useAsync?: boolean,
): Promise<{
  success: boolean;
  message: string;
  session_id?: string;
  data?: unknown;
}> {
  return safeInvoke<{
    success: boolean;
    message: string;
    session_id?: string;
    data?: unknown;
  }>('switch_session', {
    request: { session_id: sessionId, use_async: useAsync },
  });
}

/**
 * A default export containing all the client functions, for compatibility with older code.
 * @deprecated It is recommended to use named imports instead.
 */
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
  getServiceContext,
  greet,
};

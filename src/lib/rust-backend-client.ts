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
import type { Message } from '@/models/chat';
import type { Page } from '@/lib/db/types';

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

// ========================================
// Message Management
// ========================================

/**
 * Deserialize a message from the Rust backend format to frontend format.
 * Parses JSON fields and converts timestamps to Date objects.
 */
function deserializeMessage(rustMsg: Record<string, unknown>): Message {
  return {
    id: rustMsg.id as string,
    sessionId: rustMsg.sessionId as string,
    threadId: (rustMsg.threadId as string) || (rustMsg.sessionId as string), // Fallback to sessionId for backward compatibility
    role: rustMsg.role as 'user' | 'assistant' | 'system' | 'tool',
    content: JSON.parse(rustMsg.content as string),
    tool_calls: rustMsg.toolCalls
      ? JSON.parse(rustMsg.toolCalls as string)
      : undefined,
    tool_call_id: rustMsg.toolCallId as string | undefined,
    isStreaming: rustMsg.isStreaming as boolean | undefined,
    thinking: rustMsg.thinking as string | undefined,
    thinkingSignature: rustMsg.thinkingSignature as string | undefined,
    assistantId: rustMsg.assistantId as string | undefined,
    attachments: rustMsg.attachments
      ? JSON.parse(rustMsg.attachments as string)
      : undefined,
    tool_use: rustMsg.toolUse
      ? JSON.parse(rustMsg.toolUse as string)
      : undefined,
    createdAt: new Date(rustMsg.createdAt as number),
    updatedAt: new Date(rustMsg.updatedAt as number),
    source: rustMsg.source as 'assistant' | 'ui' | undefined,
    error: rustMsg.error ? JSON.parse(rustMsg.error as string) : undefined,
  };
}

/**
 * Retrieves a paginated list of messages for a specific thread in a session.
 * @param sessionId The ID of the session
 * @param threadId The ID of the thread (defaults to sessionId for top thread)
 * @param page The page number to retrieve (1-indexed)
 * @param pageSize The number of messages per page
 * @returns A promise that resolves to a Page of messages
 */
export async function getMessagesPageForSession(
  sessionId: string,
  threadId: string,
  page: number,
  pageSize: number,
): Promise<Page<Message>> {
  if (!sessionId || !threadId) {
    throw new Error('sessionId and threadId are required');
  }

  const result = await safeInvoke<Page<Record<string, unknown>>>(
    'messages_get_page',
    {
      sessionId,
      threadId,
      page,
      pageSize,
    },
  );

  // Deserialize messages from Rust format
  return {
    ...result,
    items: result.items.map(deserializeMessage),
  };
}

/**
 * Inserts or updates multiple messages in a single transaction.
 * @param messages An array of messages to upsert
 * @returns A promise that resolves when the operation completes
 */
export async function upsertMessages(messages: Message[]): Promise<void> {
  // Validate all messages have sessionId and threadId
  for (const message of messages) {
    if (!message.sessionId || message.sessionId.trim() === '') {
      throw new Error(
        `Cannot upsert message: missing or empty sessionId for message ${message.id}`,
      );
    }
    if (!message.threadId || message.threadId.trim() === '') {
      throw new Error(
        `Cannot upsert message: missing or empty threadId for message ${message.id}`,
      );
    }
  }

  // Convert Message objects to match Rust expectations
  const rustMessages = messages.map((msg) => ({
    id: msg.id,
    sessionId: msg.sessionId,
    threadId: msg.threadId,
    role: msg.role,
    content: JSON.stringify(msg.content),
    toolCalls: msg.tool_calls ? JSON.stringify(msg.tool_calls) : null,
    toolCallId: msg.tool_call_id || null,
    isStreaming: msg.isStreaming || null,
    thinking: msg.thinking || null,
    thinkingSignature: msg.thinkingSignature || null,
    assistantId: msg.assistantId || null,
    attachments: msg.attachments ? JSON.stringify(msg.attachments) : null,
    toolUse: msg.tool_use ? JSON.stringify(msg.tool_use) : null,
    createdAt: msg.createdAt ? msg.createdAt.getTime() : Date.now(),
    updatedAt: msg.updatedAt ? msg.updatedAt.getTime() : Date.now(),
    source: msg.source || null,
    error: msg.error ? JSON.stringify(msg.error) : null,
  }));

  return safeInvoke<void>('messages_upsert_many', { messages: rustMessages });
}

/**
 * Inserts or updates a single message.
 * @param message The message to upsert
 * @returns A promise that resolves when the operation completes
 */
export async function upsertMessage(message: Message): Promise<void> {
  // Validate message has sessionId
  if (!message.sessionId || message.sessionId.trim() === '') {
    throw new Error(
      `Cannot upsert message: missing or empty sessionId for message ${message.id}`,
    );
  }

  const rustMessage = {
    id: message.id,
    sessionId: message.sessionId,
    role: message.role,
    content: JSON.stringify(message.content),
    toolCalls: message.tool_calls ? JSON.stringify(message.tool_calls) : null,
    toolCallId: message.tool_call_id || null,
    isStreaming: message.isStreaming || null,
    thinking: message.thinking || null,
    thinkingSignature: message.thinkingSignature || null,
    assistantId: message.assistantId || null,
    attachments: message.attachments
      ? JSON.stringify(message.attachments)
      : null,
    toolUse: message.tool_use ? JSON.stringify(message.tool_use) : null,
    createdAt: message.createdAt ? message.createdAt.getTime() : Date.now(),
    updatedAt: message.updatedAt ? message.updatedAt.getTime() : Date.now(),
    source: message.source || null,
    error: message.error ? JSON.stringify(message.error) : null,
  };

  return safeInvoke<void>('messages_upsert', { message: rustMessage });
}

/**
 * Deletes a single message by ID.
 * @param messageId The ID of the message to delete
 * @returns A promise that resolves when the operation completes
 */
export async function deleteMessage(messageId: string): Promise<void> {
  return safeInvoke<void>('messages_delete', { messageId });
}

/**
 * Deletes all messages for a specific session.
 * @param sessionId The ID of the session
 * @returns A promise that resolves when the operation completes
 */
export async function deleteAllMessagesForSession(
  sessionId: string,
): Promise<void> {
  return safeInvoke<void>('messages_delete_all_for_session', { sessionId });
}

// ========================================
// Message Search
// ========================================

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
 * Searches messages using BM25 full-text search.
 * @param query Search query string
 * @param sessionId Session ID to search within (optional - omit for global search)
 * @param page Page number (1-indexed, default: 1)
 * @param pageSize Number of results per page (default: 25)
 * @returns A promise that resolves to a Page of search results
 */
export async function searchMessages(
  query: string,
  sessionId?: string,
  page = 1,
  pageSize = 25,
): Promise<Page<MessageSearchResult>> {
  const result = await safeInvoke<Page<Record<string, unknown>>>(
    'messages_search',
    {
      query,
      sessionId: sessionId || null,
      page,
      pageSize,
    },
  );

  // Deserialize search results from Rust format (serde rename_all = "camelCase")
  return {
    ...result,
    items: result.items.map((r) => {
      const timestamp = r.createdAt as number | undefined;
      // Validate timestamp before creating Date object
      const date =
        typeof timestamp === 'number' &&
        !isNaN(timestamp) &&
        isFinite(timestamp)
          ? new Date(timestamp)
          : new Date(0);

      return {
        messageId: r.messageId as string,
        sessionId: r.sessionId as string,
        score: r.score as number,
        snippet: r.snippet as string | undefined,
        createdAt: date,
      };
    }),
  };
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

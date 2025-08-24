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
): Promise<MCPResponse> {
  return safeInvoke<MCPResponse>('call_mcp_tool', {
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
}): Promise<MCPTool[]> {
  return safeInvoke<MCPTool[]>('list_tools_from_config', { config });
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
): Promise<MCPResponse> {
  return safeInvoke<MCPResponse>('call_builtin_tool', {
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
): Promise<MCPResponse> {
  return safeInvoke<MCPResponse>('call_tool_unified', {
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

export async function writeFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('write_file', { filePath, content });
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
  writeFile,
  getAppLogsDir,
  backupCurrentLog,
  clearCurrentLog,
  listLogFiles,
  openExternalUrl,
  greet,
};

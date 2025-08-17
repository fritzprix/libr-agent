import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';

const logger = getLogger('RustBackend');

// Type definitions for Tauri command parameters and responses
export interface MCPServerConfig {
  name: string;
  transport: string;
  [key: string]: unknown;
}

export interface MCPTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface MCPResponse {
  success: boolean;
  data?: unknown;
  error?: string;
  [key: string]: unknown;
}

// File system related types
export interface FileReadParams {
  filePath: string;
}

// Log management types
export interface LogFileBackupResult {
  backupPath: string;
}

/**
 * Type-safe hook for interacting with Rust backend commands
 * Provides centralized error handling and logging
 */
export const useRustBackend = () => {
  // Helper function for safe invoke calls with logging
  const safeInvoke = async <T>(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<T> => {
    try {
      logger.debug(`Invoking Rust command: ${command}`, args);
      const result = await invoke<T>(command, args);
      logger.debug(`Command ${command} completed successfully`);
      return result;
    } catch (error) {
      logger.error(`Command ${command} failed:`, {
        error: error instanceof Error ? error.message : String(error),
        args,
      });
      throw error;
    }
  };

  // MCP Server Management
  const startMCPServer = async (config: MCPServerConfig): Promise<string> => {
    return safeInvoke<string>('start_mcp_server', { config });
  };

  const stopMCPServer = async (serverName: string): Promise<void> => {
    return safeInvoke<void>('stop_mcp_server', { serverName });
  };

  const callMCPTool = async (
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> => {
    return safeInvoke<MCPResponse>('call_mcp_tool', {
      serverName,
      toolName,
      arguments: arguments_,
    });
  };

  const listMCPTools = async (serverName: string): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('list_mcp_tools', { serverName });
  };

  const listToolsFromConfig = async (
    config: Record<string, unknown>,
  ): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('list_tools_from_config', { config });
  };

  const getConnectedServers = async (): Promise<string[]> => {
    return safeInvoke<string[]>('get_connected_servers');
  };

  const checkServerStatus = async (serverName: string): Promise<boolean> => {
    return safeInvoke<boolean>('check_server_status', { serverName });
  };

  const checkAllServersStatus = async (): Promise<Record<string, boolean>> => {
    return safeInvoke<Record<string, boolean>>('check_all_servers_status');
  };

  const listAllTools = async (): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('list_all_tools');
  };

  const getValidatedTools = async (serverName: string): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('get_validated_tools', { serverName });
  };

  const validateToolSchema = async (tool: MCPTool): Promise<void> => {
    return safeInvoke<void>('validate_tool_schema', { tool });
  };

  // Built-in Tools
  const listBuiltinServers = async (): Promise<string[]> => {
    return safeInvoke<string[]>('list_builtin_servers');
  };

  const listBuiltinTools = async (): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('list_builtin_tools');
  };

  const callBuiltinTool = async (
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> => {
    return safeInvoke<MCPResponse>('call_builtin_tool', {
      serverName,
      toolName,
      arguments: arguments_,
    });
  };

  // Unified Tools API
  const listAllToolsUnified = async (): Promise<MCPTool[]> => {
    return safeInvoke<MCPTool[]>('list_all_tools_unified');
  };

  const callToolUnified = async (
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> => {
    return safeInvoke<MCPResponse>('call_tool_unified', {
      serverName,
      toolName,
      arguments: arguments_,
    });
  };

  // File System Operations
  const readFile = async (filePath: string): Promise<number[]> => {
    return safeInvoke<number[]>('read_file', { filePath });
  };

  // Log Management
  const getAppLogsDir = async (): Promise<string> => {
    return safeInvoke<string>('get_app_logs_dir');
  };

  const backupCurrentLog = async (): Promise<string> => {
    return safeInvoke<string>('backup_current_log');
  };

  const clearCurrentLog = async (): Promise<void> => {
    return safeInvoke<void>('clear_current_log');
  };

  const listLogFiles = async (): Promise<string[]> => {
    return safeInvoke<string[]>('list_log_files');
  };

  // External URL handling
  const openExternalUrl = async (url: string): Promise<void> => {
    return safeInvoke<void>('open_external_url', { url });
  };

  // Utility function for basic greeting (example)
  const greet = async (name: string): Promise<string> => {
    return safeInvoke<string>('greet', { name });
  };

  return {
    // MCP Server Management
    startMCPServer,
    stopMCPServer,
    callMCPTool,
    listMCPTools,
    listToolsFromConfig,
    getConnectedServers,
    checkServerStatus,
    checkAllServersStatus,
    listAllTools,
    getValidatedTools,
    validateToolSchema,

    // Built-in Tools
    listBuiltinServers,
    listBuiltinTools,
    callBuiltinTool,

    // Unified Tools API
    listAllToolsUnified,
    callToolUnified,

    // File System Operations
    readFile,

    // Log Management
    getAppLogsDir,
    backupCurrentLog,
    clearCurrentLog,
    listLogFiles,

    // External URL handling
    openExternalUrl,

    // Utility
    greet,
  };
};

export type RustBackend = ReturnType<typeof useRustBackend>;

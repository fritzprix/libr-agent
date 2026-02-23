import * as client from '@/lib/backend';
import type {
  MCPTool,
  MCPServerConfig,
  MCPConfig,
  SamplingOptions,
  SamplingResponse,
} from '@/lib/mcp';
import type { MCPResponse } from '@/lib/mcp/protocol';
import type { MCPResult } from '@/lib/mcp/protocol/response';
import type {
  ServiceContext,
  ServiceContextOptions,
} from '@/features/mcp/types';

// Workspace types
export type { WorkspaceFileItem } from '@/lib/backend';

// File system related types
export interface FileReadParams {
  filePath: string;
}

// Log management types
export interface LogFileBackupResult {
  backupPath: string;
}

/**
 * Strongly-typed interface for Rust backend operations
 * This ensures type safety between Rust backend and React frontend
 */
export interface RustBackendAPI {
  // Workspace Management
  listWorkspaceFiles: (
    path?: string,
    sessionId?: string,
  ) => Promise<client.WorkspaceFileItem[]>;
  openWorkspaceFileWithDefaultApp: (
    filePath: string,
    sessionId?: string,
  ) => Promise<void>;

  // MCP Server Management
  startMCPServer: (config: MCPServerConfig) => Promise<string>;
  stopMCPServer: (serverName: string) => Promise<void>;
  callMCPTool: (
    serverName: string,
    toolName: string,
    args: Record<string, unknown>,
    requestId?: string,
  ) => Promise<MCPResponse<unknown>>;
  listMCPTools: (serverName: string) => Promise<MCPTool[]>;
  listToolsFromConfig: (
    config: MCPConfig,
  ) => Promise<Record<string, MCPTool[]>>;
  getConnectedServers: () => Promise<string[]>;
  checkServerStatus: (serverName: string) => Promise<boolean>;
  checkAllServersStatus: () => Promise<Record<string, boolean>>;
  listAllTools: () => Promise<MCPTool[]>;
  getValidatedTools: (serverName: string) => Promise<MCPTool[]>;
  validateToolSchema: (tool: MCPTool) => Promise<void>;

  // Built-in Tools
  listBuiltinServers: () => Promise<string[]>;
  listBuiltinTools: (serverName: string) => Promise<MCPTool[]>;
  listBuiltinServersWithMetadata: () => Promise<client.BuiltinServerInfo[]>;
  listAvailableBuiltinServerDefinitions: () => Promise<
    client.BuiltinServerInfo[]
  >;
  callBuiltinTool: (
    serverName: string,
    toolName: string,
    args: Record<string, unknown>,
  ) => Promise<MCPResponse<unknown>>;

  // Unified Tools API
  listAllToolsUnified: () => Promise<MCPTool[]>;

  // File System Operations (returns number[] representing bytes)
  registerDroppedFiles: (paths: string[]) => Promise<void>;
  readDroppedFile: (filePath: string) => Promise<number[]>;
  writeFile: (filePath: string, content: number[]) => Promise<void>;

  // Log Management
  getAppLogsDir: () => Promise<string>;
  backupCurrentLog: () => Promise<string>;
  clearCurrentLog: () => Promise<void>;
  listLogFiles: () => Promise<string[]>;

  // External URL handling
  openExternalUrl: (url: string) => Promise<void>;

  // File Download Operations
  downloadWorkspaceFile: (
    filePath: string,
    sessionId: string,
  ) => Promise<string>;
  exportAndDownloadZip: (
    files: string[],
    packageName: string,
    sessionId: string,
  ) => Promise<string>;

  // Service Context
  getServiceContext: (
    serverId: string,
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;

  // Utility
  greet: (name: string) => Promise<string>;

  /**
   * Call a builtin tool for a specific agent session
   * @returns MCPResult with typed structured content
   */
  agentCallBuiltinTool: <T = unknown>(
    sessionId: string,
    toolName: string,
    args: Record<string, unknown>,
  ) => Promise<MCPResult<T>>;

  // Additional methods that may be used by legacy code
  sampleFromModel: (
    serverName: string,
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
}

// Define the API object outside the hook to ensure referential stability
const backendAPI: RustBackendAPI = {
  // Workspace Management
  listWorkspaceFiles: client.listWorkspaceFiles,
  openWorkspaceFileWithDefaultApp: client.openWorkspaceFileWithDefaultApp,

  // MCP Server Management
  startMCPServer: client.startServer,
  stopMCPServer: client.stopServer,
  callMCPTool: client.callTool,
  listMCPTools: client.listTools,
  listToolsFromConfig: client.listToolsFromConfig,
  getConnectedServers: client.getConnectedServers,
  checkServerStatus: client.checkServerStatus,
  checkAllServersStatus: client.checkAllServersStatus,
  listAllTools: client.listAllTools,
  getValidatedTools: client.getValidatedTools,
  validateToolSchema: client.validateToolSchema,

  // Built-in Tools
  listBuiltinServers: client.listBuiltinServers,
  listBuiltinTools: client.listBuiltinTools,
  listBuiltinServersWithMetadata: client.listBuiltinServersWithMetadata,
  listAvailableBuiltinServerDefinitions:
    client.listAvailableBuiltinServerDefinitions,
  callBuiltinTool: client.callBuiltinTool,

  // Unified Tools API
  listAllToolsUnified: client.listAllToolsUnified,

  // File System Operations
  registerDroppedFiles: client.registerDroppedFiles,
  readDroppedFile: client.readDroppedFile,
  writeFile: client.writeFile,

  // Log Management
  getAppLogsDir: client.getAppLogsDir,
  backupCurrentLog: client.backupCurrentLog,
  clearCurrentLog: client.clearCurrentLog,
  listLogFiles: client.listLogFiles,

  // External URL handling
  openExternalUrl: client.openExternalUrl,

  // File Download Operations
  downloadWorkspaceFile: client.downloadWorkspaceFile,
  exportAndDownloadZip: client.exportAndDownloadZip,

  // Service Context
  getServiceContext: client.getServiceContext,

  // Utility
  greet: client.greet,
  agentCallBuiltinTool: client.agentCallBuiltinTool,

  // Additional methods that may be used by legacy code
  sampleFromModel: client.sampleFromModel,
};

/**
 * React hook wrapping the shared Rust backend client
 * Provides a React-friendly API while delegating to the shared implementation
 *
 * @returns Strongly-typed interface to Rust backend operations
 */
export const useRustBackend = (): RustBackendAPI => {
  return backendAPI;
};

import * as client from '@/lib/rust-backend-client';

// Re-export types for backward compatibility
export interface MCPServerConfig {
  name: string;
  transport: string;
  [key: string]: unknown;
}

// Workspace types
export type { WorkspaceFileItem } from '@/lib/rust-backend-client';

// File system related types
export interface FileReadParams {
  filePath: string;
}

// Log management types
export interface LogFileBackupResult {
  backupPath: string;
}

/**
 * React hook wrapping the shared Rust backend client
 * Provides a React-friendly API while delegating to the shared implementation
 */
export const useRustBackend = () => {
  return {
    // Workspace Management
    listWorkspaceFiles: client.listWorkspaceFiles,

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
    callBuiltinTool: client.callBuiltinTool,

    // Unified Tools API
    listAllToolsUnified: client.listAllToolsUnified,
    callToolUnified: client.callToolUnified,

    // File System Operations
    readFile: client.readFile,
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

    // Additional methods that may be used by legacy code
    sampleFromModel: client.sampleFromModel,
  } as const;
};

export type RustBackend = ReturnType<typeof useRustBackend>;

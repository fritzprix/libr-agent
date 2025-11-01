/**
 * @file MCP Server Configuration Types
 * @description Server configuration for both legacy and V2 formats
 */

import type { TransportConfig, OAuthConfig } from './transport';

/**
 * Server metadata (optional descriptive information)
 */
export interface ServerMetadata {
  description?: string;
  vendor?: string;
  version?: string;
}

/**
 * V2 MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
 */
export interface MCPServerConfigV2 {
  name: string;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
}

/**
 * Legacy MCP Server Configuration (stdio-only, for backward compatibility)
 */
export interface LegacyMCPServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

/**
 * Top-level MCP configuration supporting both V1 and V2 formats
 */
export interface MCPConfig {
  mcpServers?: Record<string, MCPServerConfigV2 | LegacyMCPServerConfig>;
}

/**
 * Legacy format (kept for backward compatibility)
 * @deprecated Use MCPServerConfigV2 instead
 */
export interface MCPServerConfig {
  /** The unique name of the server. */
  name: string;
  /** The command to execute to start the server. */
  command?: string;
  /** An array of arguments to pass to the command. */
  args?: string[];
  /** Environment variables to set for the server process. */
  env?: Record<string, string>;
  /** The transport protocol used to communicate with the server. */
  transport: 'stdio' | 'http' | 'websocket';
  /** The URL of the server, for http or websocket transports. */
  url?: string;
  /** The port number for http or websocket transports. */
  port?: number;
}

/**
 * Defines the possible types of MCP servers.
 */
export type MCPServerType = 'tauri' | 'webworker';

/**
 * A unified configuration for an MCP server, whether it's a Tauri-based
 * backend process or a Web Worker-based server.
 */
export interface UnifiedMCPServerConfig {
  /** The unique name of the server. */
  name: string;
  /** The type of the server. */
  type: MCPServerType;
  // Properties for Tauri-based servers
  /** The command to execute to start the server. */
  command?: string;
  /** An array of arguments to pass to the command. */
  args?: string[];
  /** Environment variables to set for the server process. */
  env?: Record<string, string>;
  /** The transport protocol used to communicate with the server. */
  transport?: 'stdio' | 'http' | 'websocket';
  /** The URL of the server. */
  url?: string;
  /** The port number of the server. */
  port?: number;
  // Properties for Web Worker-based servers
  /** The path to the worker module to load. */
  modulePath?: string;
  /** The path to the main worker script. */
  workerPath?: string;
}

/**
 * Defines the context for executing a tool in a unified MCP environment.
 */
export interface MCPToolExecutionContext {
  /** The type of server where the tool will be executed. */
  serverType: MCPServerType;
  /** The name of the server to use. */
  serverName: string;
  /** The name of the tool to execute. */
  toolName: string;
  /** The arguments to pass to the tool. */
  arguments: unknown;
  /** An optional timeout for the tool execution in milliseconds. */
  timeout?: number;
}

/**
 * Type guard to check if config is V2 format
 */
export function isMCPServerConfigV2(
  config: MCPServerConfigV2 | LegacyMCPServerConfig,
): config is MCPServerConfigV2 {
  return 'transport' in config && typeof config.transport === 'object';
}

/**
 * Convert legacy config to V2 format
 */
export function convertLegacyToV2(
  name: string,
  legacy: LegacyMCPServerConfig,
): MCPServerConfigV2 {
  return {
    name,
    transport: {
      type: 'stdio',
      command: legacy.command,
      args: legacy.args,
      env: legacy.env,
    },
  };
}

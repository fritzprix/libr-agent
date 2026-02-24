import { safeInvoke } from './core';
import type { MCPTool } from '@/lib/mcp';
import type { BuiltinServerInfo } from './types';

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
 * Lists all built-in MCP servers with their metadata.
 * Returns server name, UI metadata, and tool count for each server.
 * @returns A promise that resolves to an array of `BuiltinServerInfo` objects.
 */
export async function listBuiltinServersWithMetadata(): Promise<
  BuiltinServerInfo[]
> {
  return safeInvoke<BuiltinServerInfo[]>('list_builtin_servers_with_metadata');
}

/**
 * Lists all POSSIBLE builtin server definitions for UI configuration.
 * This returns static metadata for all builtin servers that can be used in Agent V2 sessions,
 * regardless of what's currently instantiated in the global registry.
 * Use this for showing available tools in assistant/agent configuration UI.
 * @returns A promise that resolves to an array of `BuiltinServerInfo` objects.
 */
export async function listAvailableBuiltinServerDefinitions(): Promise<
  BuiltinServerInfo[]
> {
  return safeInvoke<BuiltinServerInfo[]>(
    'list_available_builtin_server_definitions',
  );
}

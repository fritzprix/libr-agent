import { safeInvoke } from './core';
import type { MCPTool, MCPResponse } from '@/lib/mcp-types';
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

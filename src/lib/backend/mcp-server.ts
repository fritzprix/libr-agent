import { safeInvoke } from './core';
import type {
  MCPServerConfig,
  MCPTool,
  MCPResponse,
  SamplingOptions,
  SamplingResponse,
  OAuthConfig,
} from '@/lib/mcp-types';
import type { MCPConfig } from '@/models/chat';

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
export async function listToolsFromConfig(
  config: MCPConfig,
): Promise<Record<string, MCPTool[]>> {
  return safeInvoke<Record<string, MCPTool[]>>('list_tools_from_config', {
    config,
  });
}

// ============================================================================
// OAuth 2.1 Authentication Functions
// ============================================================================

/**
 * Starts an OAuth 2.1 authorization flow with PKCE for an MCP server.
 *
 * @param serverId - Unique identifier for the MCP server
 * @param config - OAuth configuration
 * @returns Tuple containing authorization URL and CSRF state token
 */
export async function startOAuthFlow(
  serverId: string,
  config: OAuthConfig,
): Promise<[string, string]> {
  return safeInvoke<[string, string]>('start_oauth_flow', {
    serverId,
    config,
  });
}

/**
 * Completes an OAuth 2.1 authorization flow by exchanging code for token.
 *
 * @param serverId - Unique identifier for the MCP server
 * @param config - OAuth configuration
 * @param authorizationCode - Code received from OAuth callback
 * @param state - CSRF state token for validation
 * @returns Success message
 */
export async function completeOAuthFlow(
  serverId: string,
  config: OAuthConfig,
  authorizationCode: string,
  state: string,
): Promise<string> {
  return safeInvoke<string>('complete_oauth_flow', {
    serverId,
    config,
    authorizationCode,
    state,
  });
}

/**
 * Checks if an OAuth token exists in the OS keychain.
 *
 * @param serverId - Unique identifier for the MCP server
 * @returns True if token exists, false otherwise
 */
export async function hasOAuthToken(serverId: string): Promise<boolean> {
  return safeInvoke<boolean>('has_oauth_token', { serverId });
}

/**
 * Retrieves a cached OAuth token from the OS keychain.
 *
 * @param serverId - Unique identifier for the MCP server
 * @returns Token if found, null otherwise
 */
export async function getOAuthToken(serverId: string): Promise<string | null> {
  return safeInvoke<string | null>('get_oauth_token', { serverId });
}

/**
 * Revokes and deletes an OAuth token from the OS keychain.
 *
 * @param serverId - Unique identifier for the MCP server
 * @returns Success message
 */
export async function revokeOAuthToken(serverId: string): Promise<string> {
  return safeInvoke<string>('revoke_oauth_token', { serverId });
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

import { safeInvoke } from './core';
import type {
  MCPTool,
  MCPResponse,
  SamplingOptions,
  SamplingResponse,
} from '@/lib/mcp';
import { createId } from '@paralleldrive/cuid2';

// ========================================
// MCP Server Management
// ========================================

/**
 * Calls a tool on a specified MCP server.
 * @param serverName The name of the server.
 * @param toolName The name of the tool to call.
 * @param args The arguments to pass to the tool.
 * @param requestId Optional request ID for tracking. If not provided, a new ID is generated.
 * @returns A promise that resolves to an `MCPResponse`.
 */
export async function callTool(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
  requestId?: string,
): Promise<MCPResponse<unknown>> {
  const id = requestId ?? createId();
  return safeInvoke<MCPResponse<unknown>>('call_mcp_tool', {
    serverName,
    toolName,
    arguments: args,
    requestId: id,
  });
}

// ============================================================================
// OAuth 2.1 Authentication Functions
// ============================================================================

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
 * Validates the schema of a single tool.
 * @param tool The `MCPTool` object to validate.
 * @returns A promise that resolves if the schema is valid, or rejects otherwise.
 */
export async function validateToolSchema(tool: MCPTool): Promise<void> {
  return safeInvoke<void>('validate_tool_schema', { tool });
}

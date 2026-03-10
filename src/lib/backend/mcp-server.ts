import { safeInvoke } from './core';
import type { MCPTool } from '@/lib/mcp';

// ========================================
// MCP Server Management
// ========================================

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

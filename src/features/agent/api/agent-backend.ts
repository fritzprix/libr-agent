import { invoke } from '@tauri-apps/api/core';
import type { AddContentMetadata } from '@/models/content-store';
import type { MCPResult } from '@/lib/mcp/protocol/response';

/**
 * Call a builtin tool specifically for a given agent session.
 * This routes the call through the session-specific proxy, ensuring isolation.
 */
export async function agentCallBuiltinTool<T = unknown>(
  sessionId: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResult<T>> {
  return invoke('agent_call_builtin_tool', {
    sessionId,
    toolName,
    args,
  });
}

/**
 * Save a file to the agent's session-specific content store.
 * This bypasses the global content store and ensures the file is
 * accessible to the agent in the correct session context.
 */
export async function saveAgentFile(
  sessionId: string,
  fileName: string,
  args: {
    content?: string;
    fileUrl?: string;
    metadata?: AddContentMetadata;
  },
): Promise<unknown> {
  // Note: The tool name must be namespaced for the proxy: builtin_{server_name}__{tool_name}
  // Server ID is 'attachments' in extract_builtin_tool_ids (tools.rs)
  const response = await agentCallBuiltinTool(
    sessionId,
    'builtin_attachments__addContent',
    {
      ...args,
      metadata: {
        ...args.metadata,
        filename: fileName, // Ensure filename is passed in metadata
      },
    },
  );

  // Unwrap structuredContent if present (std MCPResult format)
  if (
    response &&
    typeof response === 'object' &&
    'structuredContent' in response
  ) {
    return response.structuredContent;
  }

  return response;
}

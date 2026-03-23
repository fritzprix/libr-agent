import { safeInvoke } from '@/lib/backend/core';

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
  return safeInvoke<MCPResult<T>>('agent_call_builtin_tool', {
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
  const response = await safeInvoke<MCPResult>('agent_add_attachment', {
    sessionId,
    args: {
      ...args,
      metadata: {
        ...args.metadata,
        filename: fileName,
      },
    },
  });

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

export async function deleteAgentFile(
  sessionId: string,
  contentId: string,
): Promise<MCPResult> {
  return safeInvoke<MCPResult>('agent_delete_attachment', {
    sessionId,
    args: {
      contentId,
    },
  });
}

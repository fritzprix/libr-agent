import { safeInvoke } from '@/lib/backend/core';

import {
  isAttachmentItem,
  type AddAttachmentMetadata,
  type AttachmentItem,
} from '@/models/attachments';
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
 * Save a file to the agent's session-specific attachments.
 * This bypasses the global attachments and ensures the file is
 * accessible to the agent in the correct session context.
 */
export async function saveAgentFile(
  sessionId: string,
  fileName: string,
  args: {
    content?: string;
    fileUrl?: string;
    metadata?: AddAttachmentMetadata;
  },
): Promise<AttachmentItem> {
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

  const structuredContent = response.structuredContent;
  if (isAttachmentItem(structuredContent)) {
    return structuredContent;
  }

  throw new Error(
    'agent_add_attachment returned an invalid attachment payload',
  );
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

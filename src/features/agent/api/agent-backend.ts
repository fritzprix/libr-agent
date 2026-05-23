import { safeInvoke } from '@/lib/backend/core';

import {
  isAttachmentItem,
  type AddAttachmentMetadata,
  type AttachmentItem,
} from '@/models/attachments';
import type { MCPContent } from '@/lib/mcp/protocol/content';
import type { MCPResult } from '@/lib/mcp/protocol/response';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isTextContent(
  value: unknown,
): value is Extract<MCPContent, { type: 'text' }> {
  return (
    isRecord(value) && value.type === 'text' && typeof value.text === 'string'
  );
}

function extractMcpTextMessage(result: MCPResult): string | null {
  if (!Array.isArray(result.content)) {
    return null;
  }

  const messages = result.content
    .filter(isTextContent)
    .map((item) => item.text.trim())
    .filter((text) => text.length > 0);

  return messages.length > 0 ? messages.join('\n\n') : null;
}

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

  const backendMessage = extractMcpTextMessage(response);

  if (response.isError) {
    throw new Error(backendMessage ?? 'agent_add_attachment failed');
  }

  const structuredContent = response.structuredContent;
  if (isAttachmentItem(structuredContent)) {
    return structuredContent;
  }

  throw new Error(
    backendMessage
      ? `agent_add_attachment returned an invalid attachment payload: ${backendMessage}`
      : 'agent_add_attachment returned an invalid attachment payload',
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

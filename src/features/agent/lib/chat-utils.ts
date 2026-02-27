import type { Message, ToolCall } from '@/models/chat';
import type { MCPContent, MCPToolCallContent } from '@/lib/mcp';

/**
 * Computes the display content for a message bubble.
 *
 * Logic extracted from AgentMessageBubble to separate presentation from transformation.
 *
 * @param msg The primary message to display
 * @param groupedMessages Optional array of grouped messages (e.g. from tool execution groups)
 * @param groupedToolCalls Optional array of tool calls (legacy/fallback)
 * @returns Array of MCPContent to render, or undefined if no special content structure is needed
 */
export function computeDisplayContent(
  msg: Message,
  groupedMessages?: Message[],
  groupedToolCalls?: ToolCall[],
): MCPContent[] | undefined {
  if (groupedMessages && groupedMessages.length > 0) {
    return groupedMessages.flatMap((m) => {
      const originalContent = Array.isArray(m.content) ? m.content : [];
      const nonToolContent = originalContent.filter(
        (c) => c.type !== 'tool_call',
      );

      const toolContent = (m.tool_calls || []).map(
        (tc): MCPToolCallContent => ({
          type: 'tool_call',
          id: tc.id,
          name: tc.function.name,
          arguments: tc.function.arguments,
        }),
      );

      return [...nonToolContent, ...toolContent];
    });
  }

  if (groupedToolCalls) {
    const originalContent = Array.isArray(msg.content) ? msg.content : [];
    const nonToolContent = originalContent.filter(
      (c) => c.type !== 'tool_call',
    );

    const toolContent = groupedToolCalls.map(
      (tc): MCPToolCallContent => ({
        type: 'tool_call',
        id: tc.id,
        name: tc.function.name,
        arguments: tc.function.arguments,
      }),
    );

    return [...nonToolContent, ...toolContent];
  }

  return undefined;
}

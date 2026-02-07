import { useMemo, useEffect, useRef } from 'react';
import type {
  MCPContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { Message } from '@/models/chat';

export type RenderItem =
  | MCPContent
  | { type: 'tool_group_block'; items: MCPToolCallContent[] };

export function useContentGrouping(
  content: MCPContent[] | undefined,
  message: Message | undefined,
) {
  // Determine content source: prefer explicit 'content' prop, then fall back to message.content, then empty array
  // V2 Fix: Prioritize explicit 'content' prop if provided (e.g. for grouped tool calls)
  const finalContent: MCPContent[] = content || message?.content || [];

  // Group consecutive tool calls into blocks for display
  const renderItems = useMemo(() => {
    const items: RenderItem[] = [];
    let currentToolGroup: MCPToolCallContent[] = [];

    finalContent.forEach((item) => {
      if (item.type === 'tool_call') {
        currentToolGroup.push(item as MCPToolCallContent);
      } else {
        if (currentToolGroup.length > 0) {
          items.push({
            type: 'tool_group_block',
            items: [...currentToolGroup],
          });
          currentToolGroup = [];
        }
        items.push(item);
      }
    });

    // Flush remaining tool group
    if (currentToolGroup.length > 0) {
      items.push({ type: 'tool_group_block', items: [...currentToolGroup] });
    }

    // Fallback: If no thinking content found but message.thinking exists (e.g. from backend persistence normalization),
    // inject it at the start to ensure it is displayed.
    const hasThinkingContent = finalContent.some((c) => c.type === 'thinking');
    if (
      !hasThinkingContent &&
      message?.thinking &&
      message.thinking.length > 0
    ) {
      items.unshift({
        type: 'thinking',
        thinking: message.thinking,
        thinkingTime: message.thinkingTime,
      } as MCPThinkingContent);
    }

    return items;
  }, [finalContent, message?.thinking, message?.thinkingTime]);

  // Keep latest content in a ref to avoid recreating callbacks on each render (if used elsewhere)
  // This was in the original component, but seems to be used mainly for handleUIAction extracting service info.
  // We will return the ref as well.
  const contentRef = useRef<MCPContent[]>(finalContent);
  useEffect(() => {
    contentRef.current = finalContent;
  }, [finalContent]);

  return { renderItems, contentRef };
}

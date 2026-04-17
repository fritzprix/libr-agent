import type {
  MCPContent,
  MCPToolCallContent,
  MCPThinkingContent,
} from '@/lib/mcp';
import type { Message } from '@/models/chat';
import type { RenderItem } from '../types';

export function groupContent(
  finalContent: MCPContent[],
  message?: Message,
): RenderItem[] {
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
  if (!hasThinkingContent && message?.thinking && message.thinking.length > 0) {
    items.unshift({
      type: 'thinking',
      thinking: message.thinking,
      thinkingTime: message.thinkingTime,
    } as MCPThinkingContent);
  }

  return items;
}

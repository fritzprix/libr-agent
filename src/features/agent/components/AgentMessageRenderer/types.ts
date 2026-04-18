import type { MCPContent, MCPToolCallContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';

export interface AgentMessageRendererProps {
  content?: MCPContent[];
  message?: Message;
  className?: string;
  /** Allow resource blocks to expand to their content height (no internal scroll) */
  expandResources?: boolean;
  /** Map of tool call ID to result message (for unified rendering) */
  toolResultsMap?: Map<string, Message>;
}

export interface ToolGroupBlock {
  type: 'tool_group_block';
  items: MCPToolCallContent[];
}

export type RenderItem = MCPContent | ToolGroupBlock;

// Helper type to avoid implicit any in markdown components
export type MarkdownReflessProps<T extends React.ElementType> =
  React.ComponentPropsWithoutRef<T> & {
    node?: unknown;
  };

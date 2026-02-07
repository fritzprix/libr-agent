import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';

export interface AgentMessageRendererProps {
  content?: MCPContent[];
  message?: Message;
  className?: string;
  /** Allow resource blocks to expand to their content height (no internal scroll) */
  expandResources?: boolean;
  /** Map of tool call ID to result message (for unified rendering) */
  toolResultsMap?: Map<string, Message>;
}

import { memo, useMemo } from 'react';
import { cn } from '@/lib/utils';
import type { Message, ToolCall } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import { Paperclip, FileText } from 'lucide-react';
import { AgentMessageRenderer } from './AgentMessageRenderer';
import { computeDisplayContent } from '@/features/agent/lib/chat-utils';

interface AgentMessageBubbleProps {
  message: Message;
  assistantName?: string;
  toolResultsMap?: Map<string, Message>;
  groupedToolCalls?: ToolCall[];
  groupedMessages?: Message[];
  isPending?: boolean;
  /**
   * When true, this bubble represents a group of failed tool results.
   * Render it like normal tool output, but with subtle warning/error semantics
   * (iconography/colors) for clear visual distinction.
   */
  toolErrorGroup?: boolean;
}

function AgentMessageBubbleImpl({
  message: msg,
  assistantName,
  toolResultsMap,
  groupedToolCalls,
  groupedMessages,
  isPending = false,
  toolErrorGroup = false,
}: AgentMessageBubbleProps) {
  // Construct display content:
  // If groupedMessages is present (new logic), we interleave content from all messages.
  // If only groupedToolCalls is present (legacy/fallback), we use the old logic.
  const displayContent: MCPContent[] | undefined = useMemo(() => {
    return computeDisplayContent(msg, groupedMessages, groupedToolCalls);
  }, [groupedMessages, groupedToolCalls, msg]);

  return (
    <div className="px-4 py-2">
      <div
        className={cn(
          'flex',
          msg.role === 'user' ? 'justify-end' : 'justify-start',
        )}
      >
        <div
          className={cn(
            'relative max-w-[85%] md:max-w-2xl p-3 rounded-lg flex flex-col',
            msg.role === 'user'
              ? isPending
                ? 'bg-primary/50 text-primary-foreground opacity-70 border-2 border-dashed border-primary/40'
                : 'bg-primary text-primary-foreground'
              : toolErrorGroup
                ? 'bg-destructive/5 text-secondary-foreground border border-destructive/20'
                : 'bg-secondary text-secondary-foreground',
            // Add custom utility to ensure links inside are visible
            msg.role === 'user'
              ? '[&_a]:text-primary-foreground'
              : '[&_a]:text-primary',
          )}
        >
          <div className="text-xs font-semibold mb-1 opacity-70">
            {msg.role === 'assistant'
              ? assistantName || 'ASSISTANT'
              : msg.role === 'user'
                ? isPending
                  ? 'You (queued)'
                  : 'You'
                : msg.role.toUpperCase()}
          </div>
          <div className="whitespace-pre-wrap min-w-0">
            {/* File Attachments Display */}
            {msg.attachments && msg.attachments.length > 0 && (
              <div className="mb-3 p-3 bg-background/10 rounded-lg border border-current/10">
                <div className="text-sm mb-2 font-medium flex items-center gap-2 opacity-90">
                  <Paperclip className="w-4 h-4" />
                  <span>
                    {msg.attachments.length} file
                    {msg.attachments.length > 1 ? 's' : ''} attached
                  </span>
                </div>
                <ul className="space-y-2" aria-label="Attached files">
                  {msg.attachments.map((attachment) => (
                    <li
                      key={attachment.contentId}
                      className="flex items-center justify-between p-2 bg-background/20 rounded border border-current/10"
                    >
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <FileText className="w-3 h-3 opacity-70 flex-shrink-0" />
                        <span className="text-xs font-medium truncate">
                          {attachment.filename}
                        </span>
                        <span className="text-xs opacity-60 whitespace-nowrap">
                          ({Math.round(attachment.size / 1024)}KB)
                        </span>
                      </div>
                      <div className="text-xs opacity-50 whitespace-nowrap ml-2">
                        {attachment.lineCount} lines
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {(displayContent && displayContent.length > 0) ||
            (msg.content && msg.content.length > 0) ||
            msg.thinking ||
            msg.isStreaming ? (
              <>
                {/* Unified Rendering: AgentMessageRenderer handles all content types including thinking and tools */}
                <AgentMessageRenderer
                  content={displayContent || msg.content}
                  message={msg}
                  toolResultsMap={toolResultsMap}
                />
              </>
            ) : (
              <span className="text-muted-foreground italic">No content</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

const arePropsEqual = (
  prev: AgentMessageBubbleProps,
  next: AgentMessageBubbleProps,
) => {
  if (
    prev.message !== next.message ||
    prev.assistantName !== next.assistantName ||
    prev.isPending !== next.isPending ||
    prev.toolErrorGroup !== next.toolErrorGroup ||
    prev.groupedMessages !== next.groupedMessages ||
    prev.groupedToolCalls !== next.groupedToolCalls
  ) {
    return false;
  }

  // If toolResultsMap reference is unchanged, no need to dig deeper.
  if (prev.toolResultsMap === next.toolResultsMap) {
    return true;
  }

  // Collect all tool call IDs that this bubble may actually render results for:
  // - Direct tool calls passed via groupedToolCalls
  // - Tool calls embedded in groupedMessages (used when groupedMessages is the source of truth)
  const relevantIds = new Set<string>();

  if (next.groupedToolCalls) {
    for (const call of next.groupedToolCalls) {
      relevantIds.add(call.id);
    }
  }

  if (next.groupedMessages) {
    for (const message of next.groupedMessages) {
      if (message.tool_calls) {
        for (const call of message.tool_calls) {
          relevantIds.add(call.id);
        }
      }
    }
  }

  // If this bubble has no tool calls at all, it does not depend on toolResultsMap.
  // Skip the re-render (e.g. tool_error_group bubbles that receive no toolResultsMap).
  if (relevantIds.size === 0) {
    return true;
  }

  // Re-render only if a result that THIS bubble renders has actually changed.
  for (const id of relevantIds) {
    if (prev.toolResultsMap?.get(id) !== next.toolResultsMap?.get(id)) {
      return false;
    }
  }

  return true;
};

// Memoized to prevent unnecessary re-renders of chat bubbles when unrelated state changes (e.g. streaming, scrolling)
export const AgentMessageBubble = memo(AgentMessageBubbleImpl, arePropsEqual);

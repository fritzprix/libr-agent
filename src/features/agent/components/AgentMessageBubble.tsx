import { memo, useMemo } from 'react';
import { cn } from '@/lib/utils';
import type { Message, ToolCall } from '@/models/chat';
import type { MCPContent, MCPToolCallContent } from '@/lib/mcp';
import { Paperclip, FileText } from 'lucide-react';
import { AgentMessageRenderer } from './AgentMessageRenderer';

interface AgentMessageBubbleProps {
  message: Message;
  getAssistantName?: (msg: Message) => string;
  toolResultsMap?: Map<string, Message>;
  groupedToolCalls?: ToolCall[];
  groupedMessages?: Message[];
  pendingMessageIds?: ReadonlySet<string>; // Changed from array to Set for O(1) lookups
}

function AgentMessageBubbleImpl({
  message: msg,
  getAssistantName,
  toolResultsMap,
  groupedToolCalls,
  groupedMessages,
  pendingMessageIds, // Changed from pendingMessages array to Set
}: AgentMessageBubbleProps) {
  // Check if message is pending using O(1) Set lookup instead of O(n) array search
  const isPending = useMemo(
    () => pendingMessageIds?.has(msg.id) ?? false,
    [pendingMessageIds, msg.id],
  );

  // Construct display content:
  // If groupedMessages is present (new logic), we interleave content from all messages.
  // If only groupedToolCalls is present (legacy/fallback), we use the old logic.
  const displayContent: MCPContent[] | undefined = useMemo(() => {
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
  }, [groupedMessages, groupedToolCalls, msg.content]);

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
              : 'bg-secondary text-secondary-foreground',
          )}
        >
          <div className="text-xs font-semibold mb-1 opacity-70">
            {msg.role === 'assistant'
              ? getAssistantName?.(msg) || 'ASSISTANT'
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

// Memoized to prevent unnecessary re-renders of chat bubbles when unrelated state changes (e.g. streaming, scrolling)
export const AgentMessageBubble = memo(AgentMessageBubbleImpl);

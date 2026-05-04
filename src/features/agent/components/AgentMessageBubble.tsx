import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui/badge';
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

interface ChannelBubbleMetadata {
  serverName?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function getChannelBubbleMetadata(
  metadata: Message['metadata'],
): ChannelBubbleMetadata | null {
  if (!isRecord(metadata) || !isRecord(metadata.channel)) {
    return null;
  }

  const { serverName } = metadata.channel;
  return {
    serverName: typeof serverName === 'string' ? serverName : undefined,
  };
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
  const { t } = useTranslation();

  // Construct display content:
  // If groupedMessages is present (new logic), we interleave content from all messages.
  // If only groupedToolCalls is present (legacy/fallback), we use the old logic.
  const displayContent: MCPContent[] | undefined = useMemo(() => {
    return computeDisplayContent(msg, groupedMessages, groupedToolCalls);
  }, [groupedMessages, groupedToolCalls, msg]);

  const hasUIResource = useMemo(() => {
    const contents = displayContent || msg.content || [];
    return contents.some(
      (item) =>
        'type' in item && (item as { type: string }).type === 'resource',
    );
  }, [displayContent, msg.content]);

  const channelMetadata = useMemo(
    () => getChannelBubbleMetadata(msg.metadata),
    [msg.metadata],
  );

  const isChannelMessage = msg.source === 'channel' || channelMetadata !== null;
  const isStandardUserMessage = msg.role === 'user' && !isChannelMessage;

  return (
    <div className="px-4 py-2">
      <div
        className={cn(
          'flex',
          isStandardUserMessage ? 'justify-end' : 'justify-start',
          hasUIResource && 'w-full',
        )}
      >
        <div
          className={cn(
            'relative p-3 rounded-lg flex flex-col',
            hasUIResource ? 'w-full max-w-full' : 'max-w-[85%] md:max-w-2xl',
            isChannelMessage
              ? 'border border-amber-500/30 bg-amber-500/10 text-secondary-foreground'
              : msg.role === 'user'
                ? isPending
                  ? 'bg-primary/50 text-primary-foreground opacity-70 border-2 border-dashed border-primary/40'
                  : 'bg-primary text-primary-foreground'
                : toolErrorGroup
                  ? 'bg-destructive/5 text-secondary-foreground border border-destructive/20'
                  : 'bg-secondary text-secondary-foreground',
            // Add custom utility to ensure links inside are visible
            isStandardUserMessage
              ? '[&_a]:text-primary-foreground'
              : '[&_a]:text-primary',
          )}
        >
          <div className="mb-1 flex flex-wrap items-center gap-2 text-xs font-semibold opacity-80">
            {isChannelMessage ? (
              <>
                <span>{t('agent.bubble.notification')}</span>
                <Badge
                  variant="outline"
                  className="border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300"
                >
                  {t('agent.bubble.channel')}
                </Badge>
                {channelMetadata?.serverName ? (
                  <Badge variant="secondary" className="bg-background/70">
                    {channelMetadata.serverName}
                  </Badge>
                ) : null}
              </>
            ) : (
              <>
                {msg.role === 'assistant'
                  ? assistantName || t('agent.bubble.assistant')
                  : msg.role === 'user'
                    ? isPending
                      ? t('agent.bubble.youQueued')
                      : t('agent.bubble.you')
                    : msg.role.toUpperCase()}
              </>
            )}
          </div>
          <div className="whitespace-pre-wrap min-w-0 font-sans">
            {/* File Attachments Display */}
            {msg.attachments && msg.attachments.length > 0 && (
              <div className="mb-3 p-3 bg-background/10 rounded-lg border border-current/10">
                <div className="text-sm mb-2 font-medium flex items-center gap-2 opacity-90">
                  <Paperclip className="w-4 h-4" />
                  <span>
                    {t('agent.bubble.filesAttached', {
                      count: msg.attachments.length,
                    })}
                  </span>
                </div>
                <ul
                  className="space-y-2"
                  aria-label={t('agent.bubble.attachedFilesAria')}
                >
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
                        {t('agent.bubble.lines', {
                          count: attachment.lineCount,
                        })}
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
              <span className="text-muted-foreground italic">
                {t('agent.bubble.noContent')}
              </span>
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
  const relevantIds: string[] = [];

  if (next.groupedToolCalls) {
    for (const call of next.groupedToolCalls) {
      relevantIds.push(call.id);
    }
  }

  if (next.groupedMessages) {
    for (const message of next.groupedMessages) {
      if (message.tool_calls) {
        for (const call of message.tool_calls) {
          relevantIds.push(call.id);
        }
      }
    }
  }

  // If this bubble has no tool calls at all, it does not depend on toolResultsMap.
  // Skip the re-render (e.g. tool_error_group bubbles that receive no toolResultsMap).
  if (relevantIds.length === 0) {
    return true;
  }

  // Re-render only if a result that THIS bubble renders has actually changed.
  const idUsageCount = new Map<string, number>();
  for (const id of relevantIds) {
    const count = idUsageCount.get(id) || 0;
    idUsageCount.set(id, count + 1);

    const key = count === 0 ? id : `${id}_dup${count}`;

    if (prev.toolResultsMap?.get(key) !== next.toolResultsMap?.get(key)) {
      return false;
    }
  }

  return true;
};

// Memoized to prevent unnecessary re-renders of chat bubbles when unrelated state changes (e.g. streaming, scrolling)
export const AgentMessageBubble = memo(AgentMessageBubbleImpl, arePropsEqual);

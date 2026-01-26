import { useCallback, memo } from 'react';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';
import { Paperclip, FileText } from 'lucide-react';
import { AgentMessageRenderer } from './AgentMessageRenderer';

interface AgentMessageBubbleProps {
  message: Message;
  getAssistantName?: (msg: Message) => string;
  toolResultsMap?: Map<string, Message>;
}

function AgentMessageBubbleImpl({
  message: msg,
  getAssistantName,
  toolResultsMap,
}: AgentMessageBubbleProps) {
  const getAssistantNameForMessage = useCallback(
    (msg: Message) => {
      if (getAssistantName) return getAssistantName(msg);
      if (msg.role === 'assistant') {
        return 'Agent';
      }
      return '';
    },
    [getAssistantName],
  );

  return (
    <div key={msg.id} className="px-4 py-2">
      <div
        className={cn(
          'flex',
          msg.role === 'user' ? 'justify-end' : 'justify-start',
        )}
      >
        <div
          className={cn(
            'inline-block max-w-[70%] p-3 rounded-lg',
            msg.role === 'user'
              ? 'bg-primary text-primary-foreground'
              : 'bg-secondary text-secondary-foreground',
          )}
        >
          <div className="text-xs font-semibold mb-1 opacity-70">
            {msg.role === 'assistant'
              ? getAssistantNameForMessage(msg) || 'ASSISTANT'
              : msg.role.toUpperCase()}
          </div>
          <div className="whitespace-pre-wrap">
            {/* File Attachments Display */}
            {msg.attachments && msg.attachments.length > 0 && (
              <div className="mb-3 p-3 bg-muted/30 rounded-lg border border-muted/20">
                <div className="text-sm mb-2 font-medium flex items-center gap-2">
                  <Paperclip className="w-4 h-4" />
                  <span>
                    {msg.attachments.length} file
                    {msg.attachments.length > 1 ? 's' : ''} attached
                  </span>
                </div>
                <div className="space-y-2">
                  {msg.attachments.map((attachment) => (
                    <div
                      key={attachment.contentId}
                      className="flex items-center justify-between p-2 bg-background/50 rounded border"
                    >
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <FileText className="w-3 h-3 text-muted-foreground flex-shrink-0" />
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
                    </div>
                  ))}
                </div>
              </div>
            )}

            {(msg.content && msg.content.length > 0) ||
            msg.thinking ||
            msg.isStreaming ? (
              <>
                {/* Unified Rendering: AgentMessageRenderer handles all content types including thinking and tools */}
                <AgentMessageRenderer
                  content={msg.content}
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

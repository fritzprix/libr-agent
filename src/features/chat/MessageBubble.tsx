import { Message } from '@/models/chat';
import React from 'react';
import { LoadingSpinner } from '../../components/ui';
import MessageBubbleRouter from './MessageBubbleRouter';

interface MessageBubbleProps {
  message: Message;
  currentAssistantName?: string;
}

const MessageBubble: React.FC<MessageBubbleProps> = ({
  message,
  currentAssistantName,
}) => {
  const isUser = message.role === 'user';
  const isTool = message.role === 'tool';
  const isAssistant = message.role === 'assistant' || message.role === 'system';

  const getBubbleStyles = () => {
    if (isUser) {
      return {
        container: 'justify-end',
        bubble: 'shadow-lg border border-primary/20',
        avatar: '🧑‍💻',
        avatarBg: 'bg-primary',
      };
    } else if (isTool) {
      return {
        container: 'justify-start',
        bubble:
          'bg-muted text-muted-foreground shadow-lg border border-muted/20',
        avatar: '🔧',
        avatarBg: 'bg-muted',
      };
    } else {
      return {
        container: 'justify-start',
        bubble:
          'bg-secondary text-secondary-foreground shadow-lg border border-secondary/20',
        avatar: '🤖',
        avatarBg: 'bg-secondary',
      };
    }
  };

  const styles = getBubbleStyles();

  const getRoleLabel = () => {
    if (isUser) return 'You';
    if (isTool) return 'Tool Output';
    if (isAssistant)
      return currentAssistantName
        ? `Agent (${currentAssistantName})`
        : 'Assistant';
    return '';
  };

  return (
    <div
      className={`flex ${styles.container} mb-8 mt-3 animate-in fade-in slide-in-from-bottom-4 duration-500`}
    >
      <div
        className={`max-w-[85%] lg:max-w-4xl ${styles.bubble} rounded-2xl px-5 py-4 backdrop-blur-sm transition-all duration-200 hover:shadow-xl`}
      >
        <div className="flex items-center gap-3 mb-3">
          <div
            className={`w-7 h-7 ${styles.avatarBg} rounded-full flex items-center justify-center text-sm shadow-sm`}
          >
            {styles.avatar}
          </div>
          <div className="flex flex-col">
            <span className="text-xs font-medium opacity-90">
              {getRoleLabel()}
            </span>
            <span className="text-xs opacity-60">
              {message.createdAt?.toLocaleString()}
            </span>
          </div>
        </div>
        {message.attachments && message.attachments.length > 0 && (
          <div className="mb-3 p-3 bg-muted/30 rounded-lg border border-muted/20">
            <div className="text-sm mb-2 font-medium flex items-center gap-2">
              <span>📎</span>
              <span>
                {message.attachments.length} file
                {message.attachments.length > 1 ? 's' : ''} attached
              </span>
            </div>
            <div className="space-y-2">
              {message.attachments.map((attachment) => (
                <div
                  key={attachment.contentId}
                  className="flex items-center justify-between p-2 bg-background/50 rounded border"
                >
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <span className="text-xs">📄</span>
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
        {message.thinking && (
          <div className="flex items-center gap-3 mt-4 p-3 bg-popover rounded-lg border border-border">
            {message.isStreaming ? <LoadingSpinner size="sm" /> : <></>}
            <span className="text-sm opacity-50 italic">
              {message.thinking}
            </span>
          </div>
        )}
        <MessageBubbleRouter message={message} />
      </div>
    </div>
  );
};

export default MessageBubble;

import React from 'react';
import { ThinkingBubble } from '../../shared';
import { MCPThinkingContent } from '@/lib/mcp';

interface ThinkingContentProps {
  thinking: MCPThinkingContent;
  isStreaming?: boolean;
}

export const ThinkingContent: React.FC<ThinkingContentProps> = ({
  thinking,
  isStreaming,
}) => {
  return (
    <div className="mb-2">
      <ThinkingBubble
        thinking={thinking.thinking}
        thinkingTime={thinking.thinkingTime}
        isStreaming={isStreaming}
      />
    </div>
  );
};

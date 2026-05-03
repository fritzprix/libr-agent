import React from 'react';
import { LoadingIndicator } from './LoadingIndicator';

interface ThinkingBubbleProps {
  /** The thinking/reasoning content to display. Can be undefined during initial streaming. */
  thinking: string | undefined;
  /** Duration of thinking in seconds */
  thinkingTime?: number;
  /** Whether the message is currently streaming */
  isStreaming?: boolean;
  /** Optional className for the container */
  className?: string;
}

/**
 * ThinkingBubble - Reusable thinking/reasoning content display
 * Used in AgentMessageBubble and AgentToolCallGroup to show
 * extended thinking content from reasoning-capable models.
 *
 * Features:
 * - Displays "Thinking Process" label with timer
 * - Shows loading animation when streaming
 * - Scrollable content area with max height
 * - Consistent styling across components
 */
export const ThinkingBubble: React.FC<ThinkingBubbleProps> = ({
  thinking,
  thinkingTime,
  isStreaming = false,
  className = '',
}) => {
  // Format time as (XXs)
  const formattedTime = thinkingTime ? `(${thinkingTime.toFixed(1)}s)` : '';

  return (
    <div
      className={`flex flex-col gap-2 p-3 bg-popover rounded-lg border border-border ${className}`}
    >
      <div className="flex items-center gap-2 text-xs font-medium opacity-70">
        {isStreaming && <LoadingIndicator size="sm" />}
        <span>Thinking Process {formattedTime}</span>
      </div>
      <div className="text-xs opacity-50 italic whitespace-pre-wrap max-h-32 overflow-y-auto">
        {thinking != null && thinking.length > 0 ? thinking : 'Thinking...'}
      </div>
    </div>
  );
};

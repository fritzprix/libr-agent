import React, { useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadingIndicator } from './LoadingIndicator';

const THINKING_BOTTOM_PIN_THRESHOLD_PX = 4;

interface ThinkingBubbleProps {
  /** The thinking/reasoning content to display. Can be undefined during initial streaming. */
  thinking: string | undefined;
  /** Duration of thinking in seconds */
  thinkingTime?: number;
  /** Whether the message is currently streaming */
  isStreaming?: boolean;
  /**
   * When false, internal auto-pin is disabled (for example when the chat list is
   * no longer following the latest message).
   */
  followChatScroll?: boolean;
  /** Optional className for the container */
  className?: string;
}

function isNearThinkingBottom(element: HTMLDivElement): boolean {
  const distanceFromBottom =
    element.scrollHeight - element.scrollTop - element.clientHeight;
  return distanceFromBottom <= THINKING_BOTTOM_PIN_THRESHOLD_PX;
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
 * - Keeps the internal scroll pinned to the latest streamed reasoning output
 * - Respects manual upward scroll inside the thinking panel
 * - Consistent styling across components
 */
export const ThinkingBubble: React.FC<ThinkingBubbleProps> = ({
  thinking,
  thinkingTime,
  isStreaming = false,
  followChatScroll = true,
  className = '',
}) => {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const userReleasedAutoPinRef = useRef(false);

  useEffect(() => {
    userReleasedAutoPinRef.current = false;
  }, [isStreaming]);

  const handleScroll = useCallback(() => {
    const element = scrollRef.current;
    if (!element || !isStreaming) {
      return;
    }

    userReleasedAutoPinRef.current = !isNearThinkingBottom(element);
  }, [isStreaming]);

  useEffect(() => {
    if (
      !isStreaming ||
      !followChatScroll ||
      !scrollRef.current ||
      userReleasedAutoPinRef.current
    ) {
      return;
    }

    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [thinking, followChatScroll, isStreaming]);

  // Format time as (XXs)
  const formattedTime = thinkingTime ? `(${thinkingTime.toFixed(1)}s)` : '';

  return (
    <div
      className={`flex flex-col gap-2 p-3 bg-popover rounded-lg border border-border ${className}`}
    >
      <div className="flex items-center gap-2 text-xs font-medium opacity-70">
        {isStreaming && <LoadingIndicator size="sm" />}
        <span>
          {formattedTime
            ? t('agent.bubble.thinkingProcessWithTime', { time: formattedTime })
            : t('agent.bubble.thinkingProcess')}
        </span>
      </div>
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="text-xs opacity-50 italic whitespace-pre-wrap max-h-32 overflow-y-auto"
      >
        {thinking != null && thinking.length > 0
          ? thinking
          : t('agent.bubble.thinking')}
      </div>
    </div>
  );
};

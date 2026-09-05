import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { LoadingIndicator } from './LoadingIndicator';

const THINKING_BOTTOM_PIN_THRESHOLD_PX = 4;
const COLLAPSED_PREVIEW_CHARS = 80;

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

function getCollapsedPreview(
  thinking: string | undefined,
  fallback: string,
): string {
  if (thinking == null || thinking.length === 0) {
    return fallback;
  }
  if (thinking.length <= COLLAPSED_PREVIEW_CHARS) {
    return thinking;
  }
  return `${thinking.slice(0, COLLAPSED_PREVIEW_CHARS)}…`;
}

/**
 * ThinkingBubble - Reusable thinking/reasoning content display
 * Used in AgentMessageBubble and AgentToolCallGroup to show
 * extended thinking content from reasoning-capable models.
 *
 * Features:
 * - Collapsed by default with truncated preview
 * - Expand/collapse toggle next to the "Thinking Process" label
 * - Shows loading animation when streaming
 * - Scrollable content area with max height when expanded
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
  const [isExpanded, setIsExpanded] = useState(() => isStreaming);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const userReleasedAutoPinRef = useRef(false);

  useEffect(() => {
    userReleasedAutoPinRef.current = false;
    if (isStreaming) {
      setIsExpanded(true);
    }
  }, [isStreaming]);

  const handleToggleExpanded = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  const handleExpand = useCallback(() => {
    setIsExpanded(true);
  }, []);

  const handleScroll = useCallback(() => {
    const element = scrollRef.current;
    if (!element || !isStreaming) {
      return;
    }

    userReleasedAutoPinRef.current = !isNearThinkingBottom(element);
  }, [isStreaming]);

  useEffect(() => {
    if (
      !isExpanded ||
      !isStreaming ||
      !followChatScroll ||
      !scrollRef.current ||
      userReleasedAutoPinRef.current
    ) {
      return;
    }

    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [thinking, followChatScroll, isStreaming, isExpanded]);

  // Format time as (XXs)
  const formattedTime = thinkingTime ? `(${thinkingTime.toFixed(1)}s)` : '';
  const thinkingFallback = t('agent.bubble.thinking');
  const preview = getCollapsedPreview(thinking, thinkingFallback);

  return (
    <div
      className={`flex w-full min-w-0 flex-col gap-2 rounded-lg border border-border bg-popover p-3 ${className}`}
    >
      <div className="flex items-center gap-2 text-xs font-medium opacity-70">
        {isStreaming && <LoadingIndicator size="sm" />}
        <button
          type="button"
          onClick={handleToggleExpanded}
          className="inline-flex items-center gap-1 rounded-sm hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          aria-expanded={isExpanded}
        >
          {isExpanded ? (
            <ChevronDown className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
          )}
          <span>
            {formattedTime
              ? t('agent.bubble.thinkingProcessWithTime', {
                  time: formattedTime,
                })
              : t('agent.bubble.thinkingProcess')}
          </span>
        </button>
      </div>
      {isExpanded ? (
        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="text-xs opacity-50 italic whitespace-pre-wrap max-h-56 overflow-y-auto transition-[max-height] duration-200"
        >
          {thinking != null && thinking.length > 0
            ? thinking
            : thinkingFallback}
        </div>
      ) : (
        <div className="flex items-start gap-2">
          <p className="flex-1 text-xs opacity-50 italic whitespace-pre-wrap line-clamp-2">
            {preview}
          </p>
          <button
            type="button"
            onClick={handleExpand}
            className="shrink-0 text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
          >
            {t('agent.bubble.expandThinking', 'Expand')}
          </button>
        </div>
      )}
    </div>
  );
};

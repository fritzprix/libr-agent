import { type ForwardedRef } from 'react';
import {
  type IndexLocationWithAlign,
  type VirtuosoHandle,
} from 'react-virtuoso';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import type { useAgentChat } from '@/context/AgentChatContext';
import {
  VISUAL_BOTTOM_THRESHOLD,
  NEAR_TOP_SCROLL_THRESHOLD,
  type BottomAlignmentPhase,
} from './types';

/**
 * Fingerprint of latest-message fields that can change chat list *structure*.
 * Streaming text length and thinking body are intentionally excluded so token
 * growth can rely on ResizeObserver instead of redundant reactive scroll calls.
 * Structural changes (new content types, tools, attachments, errors) still bump
 * the fingerprint and trigger an immediate bottom-follow pass.
 */
export function getLatestMessageScrollFingerprint(
  message: Message | undefined,
): string {
  if (!message) {
    return '';
  }

  const contentSummary = (message.content ?? [])
    .filter((item) => item.type !== 'thinking')
    .map((item) => item.type)
    .join(',');

  return [
    message.id,
    message.isStreaming ? '1' : '0',
    contentSummary,
    message.tool_calls?.length ?? 0,
    message.attachments?.length ?? 0,
    message.error ?? '',
  ].join('|');
}

/**
 * True when the latest-message update does not change list structure.
 * Height may still change (thinking/text growth); ResizeObserver owns that path.
 */
export function isLayoutNeutralLatestMessageUpdate(
  previous: Message | undefined,
  next: Message | undefined,
): boolean {
  if (!previous || !next) {
    return false;
  }

  return (
    getLatestMessageScrollFingerprint(previous) ===
    getLatestMessageScrollFingerprint(next)
  );
}

export function isThinkingOnlyLatestMessageUpdate(
  previous: Message | undefined,
  next: Message | undefined,
): boolean {
  if (!previous || !next) {
    return false;
  }

  const layoutUnchanged = isLayoutNeutralLatestMessageUpdate(previous, next);
  const thinkingChanged =
    previous.thinking !== next.thinking ||
    previous.thinkingTime !== next.thinkingTime;

  return layoutUnchanged && thinkingChanged;
}

export function getPrependedFirstItemIndex(
  current: number,
  prependCount: number,
): number {
  return Math.max(0, current - prependCount);
}

export function getInitialTopMostItemIndex(
  firstItemIndex: number,
  itemCount: number,
): IndexLocationWithAlign | number {
  return itemCount > 0
    ? {
        index: firstItemIndex + itemCount - 1,
        align: 'end',
      }
    : firstItemIndex;
}

export function getVisualBottomThreshold(): number {
  return VISUAL_BOTTOM_THRESHOLD;
}

export function shouldShowAnalysisLoader(
  latestMessage: Message | undefined,
  workflowStatus: ReturnType<typeof useAgentChat>['workflowStatus'],
): boolean {
  return (
    workflowStatus === 'busy' &&
    (latestMessage?.role !== 'assistant' ||
      (latestMessage?.role === 'assistant' &&
        !latestMessage.content?.length &&
        !latestMessage.thinking &&
        !latestMessage.tool_calls?.length))
  );
}

export function isPinnedToBottom(
  distanceFromBottom: number,
  threshold = VISUAL_BOTTOM_THRESHOLD,
): boolean {
  return distanceFromBottom <= threshold;
}

/**
 * True when the viewport is genuinely pinned to the latest content.
 *
 * - Content that fits in the viewport is both "at top" and "at bottom"; treat
 *   that as a trusted bottom so follow is not paused via reached-top.
 * - During older-message prepend, scrollHeight can briefly collapse so
 *   distanceFromBottom≈0 while scrollTop is still ~0 — reject that case.
 */
export function isTrustedVisualBottom(
  distanceFromBottom: number,
  scrollTop: number,
  options?: {
    threshold?: number;
    nearTopThreshold?: number;
    scrollHeight?: number;
    clientHeight?: number;
  },
): boolean {
  const threshold = options?.threshold ?? VISUAL_BOTTOM_THRESHOLD;
  const nearTopThreshold =
    options?.nearTopThreshold ?? NEAR_TOP_SCROLL_THRESHOLD;

  if (!isPinnedToBottom(distanceFromBottom, threshold)) {
    return false;
  }

  const scrollHeight = options?.scrollHeight;
  const clientHeight = options?.clientHeight;
  if (
    scrollHeight !== undefined &&
    clientHeight !== undefined &&
    scrollHeight <= clientHeight + threshold
  ) {
    return true;
  }

  return scrollTop > nearTopThreshold;
}

export function isBottomAlignmentActive(phase: BottomAlignmentPhase): boolean {
  return phase === 'requesting' || phase === 'verifying';
}

export function setForwardedRef<T>(ref: ForwardedRef<T>, value: T) {
  if (typeof ref === 'function') {
    ref(value);
    return;
  }

  if (ref) {
    ref.current = value;
  }
}

export function scrollFooterSentinelIntoView(sentinel: HTMLDivElement | null) {
  // Test doubles can replace the DOM node with a partial mock that lacks the
  // real method, so keep the runtime guard instead of assuming browser-only DOM.
  if (!sentinel || typeof sentinel.scrollIntoView !== 'function') {
    return;
  }

  sentinel.scrollIntoView({
    block: 'end',
    inline: 'nearest',
    behavior: 'auto',
  });
}

export function scrollVirtuosoToBottom(
  virtuoso: VirtuosoHandle | null,
  itemCount: number,
): boolean {
  if (!virtuoso || itemCount === 0) {
    return false;
  }

  virtuoso.scrollToIndex({
    index: 'LAST',
    align: 'end',
    behavior: 'auto',
  });

  return true;
}

export function renderVirtualPlaceholder() {
  return <div aria-hidden="true" className="h-px" />;
}

export function getScrollContentElement(
  scroller: HTMLDivElement | null,
): HTMLElement | null {
  const firstChild = scroller?.firstElementChild;
  return firstChild instanceof HTMLElement ? firstChild : null;
}

export function groupedMessageContainsBoundary(
  groupedMessage: GroupedMessage,
  boundaryId: string | undefined,
): boolean {
  if (!boundaryId) {
    return false;
  }

  if (groupedMessage.type === 'single') {
    return groupedMessage.message.id === boundaryId;
  }

  if (groupedMessage.type === 'tool_group') {
    return groupedMessage.coveredMessageIds.includes(boundaryId);
  }

  return groupedMessage.messages.some((message) => message.id === boundaryId);
}

export function findGroupedMessageIndexByBoundary(
  groupedMessages: GroupedMessage[],
  boundaryId: string | undefined,
): number {
  if (!boundaryId) {
    return -1;
  }

  return groupedMessages.findIndex((groupedMessage) =>
    groupedMessageContainsBoundary(groupedMessage, boundaryId),
  );
}

export function getGroupedMessageVirtuosoKey(
  groupedMessage: GroupedMessage,
): string {
  if (groupedMessage.type === 'tool_group') {
    const firstCoveredId = groupedMessage.coveredMessageIds[0] ?? 'none';
    const lastCoveredId =
      groupedMessage.coveredMessageIds[
        groupedMessage.coveredMessageIds.length - 1
      ] ?? firstCoveredId;

    return [
      'tool-group',
      groupedMessage.message.id,
      firstCoveredId,
      lastCoveredId,
      groupedMessage.coveredMessageIds.length,
      groupedMessage.toolGroup.calls.length,
    ].join(':');
  }

  if (groupedMessage.type === 'tool_error_group') {
    const lastMessageId =
      groupedMessage.messages[groupedMessage.messages.length - 1]?.id ??
      groupedMessage.message.id;

    return [
      'tool-error-group',
      groupedMessage.message.id,
      lastMessageId,
      groupedMessage.messages.length,
    ].join(':');
  }

  return `single:${groupedMessage.message.id}`;
}

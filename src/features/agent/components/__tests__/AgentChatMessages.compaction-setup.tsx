// Shared fixtures, factories, and Virtuoso mock helpers for
// AgentChatMessages compaction test suites.
//
// IMPORTANT: This module contains no vi.* calls. Vitest's hoisting rules
// require all vi.mock() and vi.hoisted() declarations to reside in the
// test file that uses them.

import React, { forwardRef, useImperativeHandle } from 'react';
import type { Mock } from 'vitest';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';

// ---------------------------------------------------------------------------
// Static message fixtures
// ---------------------------------------------------------------------------

export const baseMessage: Message = {
  id: 'assistant-1',
  sessionId: 'session-1',
  threadId: 'session-1',
  role: 'assistant',
  content: [{ type: 'text', text: 'Tool call message' }],
};

export const groupedToolMessages: Message[] = [
  {
    id: 'earlier-user',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'user',
    content: [{ type: 'text', text: 'Earlier user message' }],
  },
  baseMessage,
  {
    id: 'tool-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'tool',
    tool_call_id: 'call-1',
    content: [{ type: 'text', text: 'Tool result' }],
  },
];

// ---------------------------------------------------------------------------
// GroupedMessage factories
// ---------------------------------------------------------------------------

/**
 * Returns the default compact tool group entry used as the initial
 * groupedMessagesMock state in most tests.
 */
export function makeCompactToolGroupEntry(): GroupedMessage {
  return {
    type: 'tool_group',
    message: baseMessage,
    messages: [baseMessage],
    coveredMessageIds: ['assistant-1', 'tool-1'],
    toolGroup: {
      calls: [
        {
          id: 'call-1',
          type: 'function',
          function: { name: 'agent__compactSessionContext', arguments: '{}' },
        },
      ],
      results: [],
    },
  };
}

/** Wraps a message in a single-type GroupedMessage. */
export function makeSingleGroupEntry(message: Message): GroupedMessage {
  return {
    type: 'single',
    message,
    messages: [message],
    coveredMessageIds: [message.id],
  } as GroupedMessage;
}

/** Creates a streaming assistant message (default text: 'streaming output'). */
export function makeStreamingMessage(text = 'streaming output'): Message {
  return {
    ...baseMessage,
    id: 'assistant-stream',
    content: [{ type: 'text', text }],
    isStreaming: true,
  };
}

/** Returns a single grouped entry wrapping a streaming assistant message. */
export function makeStreamingGroupEntry(
  text = 'streaming output',
): GroupedMessage {
  return makeSingleGroupEntry(makeStreamingMessage(text));
}

// ---------------------------------------------------------------------------
// Virtuoso mock implementation
// ---------------------------------------------------------------------------

interface VirtuosoMockProps {
  components?: {
    Footer?: ({ context }: { context: unknown }) => React.ReactElement | null;
    Header?: ({ context }: { context: unknown }) => React.ReactElement | null;
    List?: (props: {
      children: React.ReactNode;
      context: unknown;
      style?: React.CSSProperties;
    }) => React.ReactElement | null;
    Scroller?: React.ComponentType<
      React.ComponentPropsWithoutRef<'div'>
    > | null;
  };
  context: unknown;
  data: GroupedMessage[];
  itemContent: (
    index: number,
    item: GroupedMessage,
  ) => React.ReactElement | null;
}

export interface AgentChatMessagesTestHarness {
  virtuosoMock: Mock;
  scrollToIndexMock: Mock;
  sessionState: {
    session: {
      id: string;
      assistant: { name: string };
    };
  };
  chatState: {
    messages: Message[];
    workflowStatus: 'idle' | 'busy';
  };
  hasVirtuosoHandle: { current: boolean };
}

/**
 * Applies the standard Virtuoso mock implementation to a vitest mock function.
 * The mock renders list items through the component's own itemContent and
 * Virtuoso component slots so that test assertions can inspect real DOM output.
 */
export function applyVirtuosoMockImpl(virtuosoMock: {
  mockImplementation: (
    fn: (props: VirtuosoMockProps) => React.ReactElement,
  ) => void;
}): void {
  virtuosoMock.mockImplementation(
    ({
      components,
      context,
      data,
      itemContent,
    }: VirtuosoMockProps): React.ReactElement => {
      const Scroller = (components?.Scroller ?? 'div') as React.ElementType<{
        children?: React.ReactNode;
      }>;
      const List = components?.List;

      const content = (
        <>
          {components?.Header ? <components.Header context={context} /> : null}
          {data.map((item, index) => (
            <div key={item.message.id}>{itemContent(index, item)}</div>
          ))}
          {components?.Footer ? <components.Footer context={context} /> : null}
        </>
      );

      return (
        <Scroller>
          {List ? (
            <List context={context} style={{}}>
              {content}
            </List>
          ) : (
            content
          )}
        </Scroller>
      );
    },
  );
}

const noop = (): void => {};

export function installMockResizeObserver(callbacks: {
  current: ResizeObserverCallback[];
}): void {
  class MockResizeObserver implements ResizeObserver {
    constructor(callback: ResizeObserverCallback) {
      callbacks.current.push(callback);
    }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }

  global.ResizeObserver = MockResizeObserver;
}

export function createAgentChatContextMock(
  chatState: AgentChatMessagesTestHarness['chatState'],
) {
  return {
    useAgentChat: () => ({
      messages: chatState.messages,
      pendingMessages: [],
      error: undefined,
      llmError: undefined,
      retryMessage: noop,
      workflowStatus: chatState.workflowStatus,
    }),
  };
}

export function createAgentSessionContextMock(
  sessionState: AgentChatMessagesTestHarness['sessionState'],
) {
  return {
    useAgentSession: () => ({
      session: sessionState.session,
      pendingApprovals: [],
      respondToToolApproval: noop,
    }),
  };
}

export const llmServiceContextMock = {
  useLLMService: () => ({
    getCompactedRange: () => ({
      toId: 'tool-1',
      summary: 'Compacted summary',
    }),
  }),
};

export const agentResourceAttachmentMock = {
  useAgentResourceAttachment: () => ({ refetchSessionFiles: noop }),
};

export const fileRefetcherMock = {
  useFileRefetcher: noop,
};

export function createMessageGroupingMock(
  groupedMessagesMock: GroupedMessage[],
) {
  return {
    useMessageGrouping: () => ({
      groupedMessages: groupedMessagesMock.slice(),
      toolResultsMap: new Map(),
    }),
  };
}

export const agentMessageBubbleMock = {
  AgentMessageBubble: () => <div>message bubble</div>,
};

export const sharedComponentsMock = {
  AnalysisLoader: () => <div>analysis loader</div>,
};

export const compactEventDividerMock = {
  CompactEventDivider: ({ summary }: { summary?: string }) => (
    <div>{summary ?? 'compact divider'}</div>
  ),
};

export const pendingApprovalWidgetMock = {
  PendingApprovalWidget: () => <div>pending approvals</div>,
};

export const errorBubbleMock = {
  ErrorBubble: () => <div>error bubble</div>,
};

export const tooltipMock = {
  Tooltip: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipProvider: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
};

export function createVirtuosoModuleMock(
  harness: Pick<
    AgentChatMessagesTestHarness,
    'hasVirtuosoHandle' | 'scrollToIndexMock' | 'virtuosoMock'
  >,
) {
  return {
    Virtuoso: forwardRef(function MockVirtuoso(props, ref) {
      useImperativeHandle(
        ref,
        () =>
          harness.hasVirtuosoHandle.current
            ? { scrollToIndex: harness.scrollToIndexMock }
            : (null as unknown as {
                scrollToIndex: AgentChatMessagesTestHarness['scrollToIndexMock'];
              }),
        [ref, harness.hasVirtuosoHandle.current],
      );
      return harness.virtuosoMock(props);
    }),
  };
}

export function resetAgentChatMessagesHarness(args: {
  harness: AgentChatMessagesTestHarness;
  groupedMessagesMock: GroupedMessage[];
  resizeObserverCallbacks: { current: ResizeObserverCallback[] };
}): void {
  const { harness, groupedMessagesMock, resizeObserverCallbacks } = args;

  harness.virtuosoMock.mockClear();
  harness.scrollToIndexMock.mockClear();
  resizeObserverCallbacks.current = [];
  harness.hasVirtuosoHandle.current = true;
  harness.sessionState.session = {
    id: 'session-1',
    assistant: { name: 'Agent' },
  };
  harness.chatState.messages = groupedToolMessages.slice(1);
  harness.chatState.workflowStatus = 'idle';
  groupedMessagesMock.splice(
    0,
    groupedMessagesMock.length,
    makeCompactToolGroupEntry(),
  );
  applyVirtuosoMockImpl(harness.virtuosoMock);
}

export function installImmediateAnimationFrameMock(): () => void {
  const originalRequestAnimationFrame = global.requestAnimationFrame;
  const originalCancelAnimationFrame = global.cancelAnimationFrame;

  global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  }) as typeof requestAnimationFrame;
  global.cancelAnimationFrame = (() =>
    undefined) as typeof cancelAnimationFrame;

  return () => {
    global.requestAnimationFrame = originalRequestAnimationFrame;
    global.cancelAnimationFrame = originalCancelAnimationFrame;
  };
}

export function installScrollIntoViewMock(
  scrollIntoView: typeof HTMLElement.prototype.scrollIntoView,
): () => void {
  const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
  HTMLElement.prototype.scrollIntoView = scrollIntoView;

  return () => {
    HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
  };
}

export function setScrollerMetrics(
  scroller: HTMLDivElement,
  metrics: {
    scrollHeight: number;
    clientHeight: number;
    scrollTop: number;
  },
): void {
  Object.defineProperty(scroller, 'scrollHeight', {
    value: metrics.scrollHeight,
    configurable: true,
  });
  Object.defineProperty(scroller, 'clientHeight', {
    value: metrics.clientHeight,
    configurable: true,
  });
  Object.defineProperty(scroller, 'scrollTop', {
    value: metrics.scrollTop,
    writable: true,
    configurable: true,
  });
}

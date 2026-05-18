import '@testing-library/jest-dom';
import { act, render, screen } from '@testing-library/react';
import React, { forwardRef, useImperativeHandle } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AgentChatMessages,
  getInitialTopMostItemIndex,
  getPrependedFirstItemIndex,
  isPinnedToBottom,
  getVisualBottomThreshold,
  shouldShowAnalysisLoader,
} from '../AgentChatMessages';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';

const baseMessage: Message = {
  id: 'assistant-1',
  sessionId: 'session-1',
  threadId: 'session-1',
  role: 'assistant',
  content: [{ type: 'text', text: 'Tool call message' }],
};

const groupedToolMessages: Message[] = [
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

const groupedMessagesMock: GroupedMessage[] = [
  {
    type: 'tool_group',
    message: baseMessage,
    messages: [baseMessage],
    coveredMessageIds: ['assistant-1', 'tool-1'],
    toolGroup: {
      calls: [
        {
          id: 'call-1',
          type: 'function',
          function: {
            name: 'agent__compactSessionContext',
            arguments: '{}',
          },
        },
      ],
      results: [],
    },
  },
];

const {
  virtuosoMock,
  scrollToIndexMock,
  sessionState,
  chatState,
  hasVirtuosoHandle,
} = vi.hoisted(() => ({
  virtuosoMock: vi.fn(),
  scrollToIndexMock: vi.fn(),
  sessionState: {
    session: { id: 'session-1', assistant: { name: 'Agent' } },
  },
  chatState: {
    messages: [] as Message[],
    workflowStatus: 'idle' as 'idle' | 'busy',
  },
  hasVirtuosoHandle: { current: true },
}));

let resizeObserverCallbacks: ResizeObserverCallback[] = [];

class MockResizeObserver implements ResizeObserver {
  constructor(callback: ResizeObserverCallback) {
    resizeObserverCallbacks.push(callback);
  }

  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

global.ResizeObserver = MockResizeObserver;

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    messages: chatState.messages,
    pendingMessages: [],
    error: undefined,
    llmError: undefined,
    retryMessage: vi.fn(),
    workflowStatus: chatState.workflowStatus,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSession: () => ({
    session: sessionState.session,
    pendingApprovals: [],
    respondToToolApproval: vi.fn(),
  }),
}));

vi.mock('@/context/LLMServiceContext', () => ({
  useLLMService: () => ({
    getCompactedRange: () => ({
      fromId: 'earlier-user',
      toId: 'tool-1',
      summary: 'Compacted summary',
    }),
  }),
}));

vi.mock('@/features/agent/hooks/useAgentResourceAttachment', () => ({
  useAgentResourceAttachment: () => ({ refetchSessionFiles: vi.fn() }),
}));

vi.mock('@/features/agent/hooks/useFileRefetcher', () => ({
  useFileRefetcher: vi.fn(),
}));

vi.mock('@/hooks/useMessageGrouping', () => ({
  useMessageGrouping: () => ({
    groupedMessages: groupedMessagesMock.slice(),
    toolResultsMap: new Map(),
  }),
}));

vi.mock('../AgentMessageBubble', () => ({
  AgentMessageBubble: () => <div>message bubble</div>,
}));

vi.mock('../shared', () => ({
  AnalysisLoader: () => <div>analysis loader</div>,
}));

vi.mock('../shared/CompactEventDivider', () => ({
  CompactEventDivider: ({ summary }: { summary?: string }) => (
    <div>{summary ?? 'compact divider'}</div>
  ),
}));

vi.mock('../PendingApprovalWidget', () => ({
  PendingApprovalWidget: () => <div>pending approvals</div>,
}));

vi.mock('@/components/shared/ErrorBubble', () => ({
  ErrorBubble: () => <div>error bubble</div>,
}));

vi.mock('@/components/ui/tooltip', () => ({
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
}));

vi.mock('react-virtuoso', () => ({
  Virtuoso: forwardRef(function MockVirtuoso(props, ref) {
    useImperativeHandle(
      ref,
      () =>
        hasVirtuosoHandle.current
          ? {
              scrollToIndex: scrollToIndexMock,
            }
          : (null as unknown as { scrollToIndex: typeof scrollToIndexMock }),
      [ref, hasVirtuosoHandle.current],
    );
    return virtuosoMock(props);
  }),
}));

describe('AgentChatMessages compaction rendering', () => {
  beforeEach(() => {
    virtuosoMock.mockClear();
    scrollToIndexMock.mockClear();
    resizeObserverCallbacks = [];
    hasVirtuosoHandle.current = true;
    sessionState.session = { id: 'session-1', assistant: { name: 'Agent' } };
    chatState.messages = groupedToolMessages.slice(1);
    chatState.workflowStatus = 'idle';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: baseMessage,
        messages: [baseMessage],
        coveredMessageIds: ['assistant-1', 'tool-1'],
        toolGroup: {
          calls: [
            {
              id: 'call-1',
              type: 'function',
              function: {
                name: 'agent__compactSessionContext',
                arguments: '{}',
              },
            },
          ],
          results: [],
        },
      },
    );
    virtuosoMock.mockImplementation(
      ({
        components,
        context,
        data,
        itemContent,
      }: {
        components?: {
          Footer?: ({ context }: { context: unknown }) => JSX.Element | null;
          Header?: ({ context }: { context: unknown }) => JSX.Element | null;
          List?: (props: {
            children: React.ReactNode;
            context: unknown;
            style?: React.CSSProperties;
          }) => JSX.Element | null;
          Scroller?: React.ComponentType<
            React.ComponentPropsWithoutRef<'div'>
          > | null;
        };
        context: unknown;
        data: GroupedMessage[];
        itemContent: (
          index: number,
          item: GroupedMessage,
        ) => JSX.Element | null;
      }) => {
        const Scroller = components?.Scroller ?? 'div';
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
  });

  it('renders the compact event when the boundary falls inside a tool group', () => {
    render(<AgentChatMessages />);

    expect(screen.getByText('Compacted summary')).toBeInTheDocument();
  });

  it('opts the chat scroller out of browser scroll anchoring', () => {
    const { container } = render(<AgentChatMessages />);

    expect(container.querySelector('.agent-chat-scrollbar')).toHaveStyle({
      overflowAnchor: 'none',
    });
  });

  it('uses the absolute firstItemIndex offset for the initial bottom position', () => {
    render(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      firstItemIndex: number;
      initialTopMostItemIndex:
        | number
        | {
            index: number;
            align: 'center' | 'end' | 'start';
          };
    };

    expect(virtuosoProps.firstItemIndex).toBe(10_000);
    expect(virtuosoProps.initialTopMostItemIndex).toEqual({
      index: 10_000,
      align: 'end',
    });
    expect(getInitialTopMostItemIndex(10_000, 1)).toEqual({
      index: 10_000,
      align: 'end',
    });
    expect(getInitialTopMostItemIndex(10_000, 3)).toEqual({
      index: 10_002,
      align: 'end',
    });
  });

  it('resets the initial firstItemIndex synchronously when the session changes', () => {
    const { rerender } = render(<AgentChatMessages />);

    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'older-tool-group',
        },
        messages: [
          {
            ...baseMessage,
            id: 'older-user',
            role: 'user',
          },
        ],
        coveredMessageIds: ['older-user'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'newer-assistant',
        },
        messages: [
          {
            ...baseMessage,
            id: 'newer-assistant',
          },
        ],
        coveredMessageIds: ['newer-assistant'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
    );

    rerender(<AgentChatMessages />);

    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'session-2-message',
          sessionId: 'session-2',
          threadId: 'session-2',
        },
        messages: [
          {
            ...baseMessage,
            id: 'session-2-message',
            sessionId: 'session-2',
            threadId: 'session-2',
          },
        ],
        coveredMessageIds: ['session-2-message'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
    );
    sessionState.session = { id: 'session-2', assistant: { name: 'Agent' } };
    rerender(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      firstItemIndex: number;
      initialTopMostItemIndex:
        | number
        | {
            index: number;
            align: 'center' | 'end' | 'start';
          };
    };

    expect(virtuosoProps.firstItemIndex).toBe(10_000);
    expect(virtuosoProps.initialTopMostItemIndex).toEqual({
      index: 10_000,
      align: 'end',
    });
  });

  it('keeps prepend index adjustments monotonic at zero instead of rebounding to list length', () => {
    expect(getPrependedFirstItemIndex(10_000, 3)).toBe(9_997);
    expect(getPrependedFirstItemIndex(2, 3)).toBe(0);
  });

  it('uses a tiny bottom threshold as scroll noise tolerance', () => {
    render(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      atBottomThreshold: number;
      followOutput: false;
    };

    expect(virtuosoProps.atBottomThreshold).toBe(getVisualBottomThreshold());
    expect(virtuosoProps.followOutput).toBe(false);
    expect(getVisualBottomThreshold()).toBe(4);
  });

  it('uses horizontal list padding without a shorthand padding override', () => {
    const { container } = render(<AgentChatMessages />);

    const list = container.querySelector('[style*="padding-left: 16px"]');
    expect(list).toHaveStyle({
      paddingLeft: '16px',
      paddingRight: '16px',
    });
    expect(list?.getAttribute('style')).not.toContain('padding: 16px');
  });

  it('shows the analysis loader only for busy empty assistant output states', () => {
    expect(shouldShowAnalysisLoader(undefined, 'idle')).toBe(false);
    expect(
      shouldShowAnalysisLoader(
        { ...baseMessage, content: [], isStreaming: false },
        'busy',
      ),
    ).toBe(true);
    expect(
      shouldShowAnalysisLoader(
        { ...baseMessage, content: [{ type: 'text', text: 'done' }] },
        'busy',
      ),
    ).toBe(false);
  });

  it('renders a minimal placeholder instead of null for empty busy messages', () => {
    chatState.messages = [{ ...baseMessage, content: [] }];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'single',
        message: { ...baseMessage, content: [] },
        messages: [{ ...baseMessage, content: [] }],
        coveredMessageIds: ['assistant-1'],
      } as GroupedMessage,
    );

    const { container } = render(<AgentChatMessages />);

    expect(container.querySelector('.h-px')).toBeInTheDocument();
  });

  it('treats only tiny distances as pinned to the bottom', () => {
    expect(isPinnedToBottom(0)).toBe(true);
    expect(isPinnedToBottom(4)).toBe(true);
    expect(isPinnedToBottom(5)).toBe(false);
  });

  it('releases the bottom latch on a deliberate upward scroll before the hard cutoff', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    let currentTime = 0;

    chatState.messages = [
      {
        ...baseMessage,
        id: 'assistant-stream',
        content: [{ type: 'text', text: 'streaming output' }],
        isStreaming: true,
      },
    ];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'single',
        message: {
          ...baseMessage,
          id: 'assistant-stream',
          content: [{ type: 'text', text: 'streaming output' }],
          isStreaming: true,
        },
        messages: [
          {
            ...baseMessage,
            id: 'assistant-stream',
            content: [{ type: 'text', text: 'streaming output' }],
            isStreaming: true,
          },
        ],
        coveredMessageIds: ['assistant-stream'],
      } as GroupedMessage,
    );

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    performanceNowSpy.mockImplementation(() => currentTime);

    try {
      const { container } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();
      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      Object.defineProperty(scroller, 'scrollHeight', {
        value: 500,
        configurable: true,
      });
      Object.defineProperty(scroller, 'clientHeight', {
        value: 100,
        configurable: true,
      });
      Object.defineProperty(scroller, 'scrollTop', {
        value: 400,
        writable: true,
        configurable: true,
      });

      currentTime = 300;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 360;

      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
    }
  });

  it('reacquires the bottom latch when the user manually returns to latest during busy streaming', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    const scrollIntoView = vi.fn();
    let currentTime = 0;

    chatState.messages = [
      {
        ...baseMessage,
        id: 'assistant-stream',
        content: [{ type: 'text', text: 'streaming output' }],
        isStreaming: true,
      },
    ];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'single',
        message: {
          ...baseMessage,
          id: 'assistant-stream',
          content: [{ type: 'text', text: 'streaming output' }],
          isStreaming: true,
        },
        messages: [
          {
            ...baseMessage,
            id: 'assistant-stream',
            content: [{ type: 'text', text: 'streaming output' }],
            isStreaming: true,
          },
        ],
        coveredMessageIds: ['assistant-stream'],
      } as GroupedMessage,
    );

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    performanceNowSpy.mockImplementation(() => currentTime);

    try {
      const { container } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();

      Object.defineProperty(scroller, 'scrollHeight', {
        value: 500,
        configurable: true,
      });
      Object.defineProperty(scroller, 'clientHeight', {
        value: 100,
        configurable: true,
      });
      Object.defineProperty(scroller, 'scrollTop', {
        value: 400,
        writable: true,
        configurable: true,
      });

      currentTime = 300;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 360;

      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      const scrollToLatestButton = screen.getByLabelText('Scroll to latest');
      expect(scrollToLatestButton).toBeInTheDocument();

      scrollIntoView.mockClear();
      currentTime = 600;

      act(() => {
        scrollToLatestButton.click();
      });

      expect(scrollIntoView).toHaveBeenCalled();
      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('keeps bottom alignment when the pinned content resizes after render', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      render(<AgentChatMessages />);

      act(() => {
        resizeObserverCallbacks.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('falls back to the footer sentinel when Virtuoso is not ready yet', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

    hasVirtuosoHandle.current = false;
    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      render(<AgentChatMessages />);

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('retries bottom alignment after delayed Virtuoso readiness changes the list height', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

    hasVirtuosoHandle.current = false;
    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      const { rerender } = render(<AgentChatMessages />);

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();

      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();
      hasVirtuosoHandle.current = true;
      rerender(<AgentChatMessages />);

      const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
        totalListHeightChanged?: (height: number) => void;
      };

      act(() => {
        virtuosoProps.totalListHeightChanged?.(512);
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('keeps retrying bottom alignment when list height grows after an early atBottom signal', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const originalSetTimeout = global.setTimeout;
    const originalClearTimeout = global.clearTimeout;
    const timeoutQueue: Array<() => void> = [];

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    global.setTimeout = ((callback: TimerHandler) => {
      if (typeof callback === 'function') {
        timeoutQueue.push(callback as () => void);
      }
      return timeoutQueue.length as unknown as ReturnType<typeof setTimeout>;
    }) as unknown as typeof setTimeout;
    global.clearTimeout = vi.fn() as typeof clearTimeout;
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      render(<AgentChatMessages />);

      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
        atBottomStateChange?: (atBottom: boolean) => void;
        totalListHeightChanged?: (height: number) => void;
      };

      act(() => {
        virtuosoProps.atBottomStateChange?.(true);
        virtuosoProps.totalListHeightChanged?.(6_899);
        virtuosoProps.atBottomStateChange?.(false);
        virtuosoProps.totalListHeightChanged?.(9_726);
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
      const scrollCallCountBeforeSettle = scrollIntoView.mock.calls.length;

      act(() => {
        while (timeoutQueue.length > 0) {
          const callback = timeoutQueue.shift();
          callback?.();
        }
      });

      act(() => {
        virtuosoProps.totalListHeightChanged?.(12_000);
      });

      expect(scrollIntoView.mock.calls.length).toBe(
        scrollCallCountBeforeSettle + 1,
      );
      expect(scrollToIndexMock).not.toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      global.setTimeout = originalSetTimeout;
      global.clearTimeout = originalClearTimeout;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('scrolls to the bottom again when the resumed session changes', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      const { rerender } = render(<AgentChatMessages />);
      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      sessionState.session = { id: 'session-2', assistant: { name: 'Agent' } };
      rerender(<AgentChatMessages />);

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('keeps following bottom while assistant text content is actively streaming', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const frameQueue: FrameRequestCallback[] = [];

    chatState.messages = [
      {
        ...baseMessage,
        id: 'assistant-stream',
        content: [{ type: 'text', text: 'streaming output' }],
        isStreaming: true,
      },
    ];
    chatState.workflowStatus = 'busy';
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      frameQueue.push(callback);
      return frameQueue.length;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();

    try {
      render(<AgentChatMessages />);
      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      const nextFrame = frameQueue.shift();
      expect(nextFrame).toBeDefined();

      act(() => {
        nextFrame?.(0);
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('keeps following bottom when streaming updates arrive during prepend stabilization', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const originalSetTimeout = global.setTimeout;
    const originalClearTimeout = global.clearTimeout;
    const frameQueue: FrameRequestCallback[] = [];
    const timeoutQueue: Array<() => void> = [];
    const scrollIntoView = vi.fn();

    chatState.messages = [
      {
        ...baseMessage,
        id: 'assistant-stream',
        content: [{ type: 'text', text: 'streaming output' }],
        isStreaming: true,
      },
    ];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'single',
        message: {
          ...baseMessage,
          id: 'assistant-stream',
          content: [{ type: 'text', text: 'streaming output' }],
          isStreaming: true,
        },
        messages: [
          {
            ...baseMessage,
            id: 'assistant-stream',
            content: [{ type: 'text', text: 'streaming output' }],
            isStreaming: true,
          },
        ],
        coveredMessageIds: ['assistant-stream'],
      } as GroupedMessage,
    );

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      frameQueue.push(callback);
      return frameQueue.length;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    global.setTimeout = ((callback: TimerHandler) => {
      if (typeof callback === 'function') {
        timeoutQueue.push(callback as () => void);
      }
      return timeoutQueue.length as unknown as ReturnType<typeof setTimeout>;
    }) as unknown as typeof setTimeout;
    global.clearTimeout = vi.fn() as typeof clearTimeout;
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      const { rerender } = render(<AgentChatMessages />);

      act(() => {
        while (frameQueue.length > 0) {
          frameQueue.shift()?.(0);
        }
      });

      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      groupedMessagesMock.splice(
        0,
        groupedMessagesMock.length,
        {
          type: 'single',
          message: {
            ...baseMessage,
            id: 'older-user',
            role: 'user',
            content: [{ type: 'text', text: 'Older user message' }],
          },
          messages: [
            {
              ...baseMessage,
              id: 'older-user',
              role: 'user',
              content: [{ type: 'text', text: 'Older user message' }],
            },
          ],
          coveredMessageIds: ['older-user'],
        } as GroupedMessage,
        {
          type: 'single',
          message: {
            ...baseMessage,
            id: 'assistant-stream',
            content: [{ type: 'text', text: 'streaming output' }],
            isStreaming: true,
          },
          messages: [
            {
              ...baseMessage,
              id: 'assistant-stream',
              content: [{ type: 'text', text: 'streaming output' }],
              isStreaming: true,
            },
          ],
          coveredMessageIds: ['assistant-stream'],
        } as GroupedMessage,
      );
      rerender(<AgentChatMessages />);

      const virtuosoPropsAfterPrepend = virtuosoMock.mock.lastCall?.[0] as {
        firstItemIndex: number;
      };

      expect(virtuosoPropsAfterPrepend.firstItemIndex).toBe(9_999);

      scrollToIndexMock.mockClear();

      chatState.messages = [
        {
          ...baseMessage,
          id: 'assistant-stream',
          content: [{ type: 'text', text: 'streaming output extended' }],
          isStreaming: true,
        },
      ];
      groupedMessagesMock.splice(
        0,
        groupedMessagesMock.length,
        {
          type: 'single',
          message: {
            ...baseMessage,
            id: 'older-user',
            role: 'user',
            content: [{ type: 'text', text: 'Older user message' }],
          },
          messages: [
            {
              ...baseMessage,
              id: 'older-user',
              role: 'user',
              content: [{ type: 'text', text: 'Older user message' }],
            },
          ],
          coveredMessageIds: ['older-user'],
        } as GroupedMessage,
        {
          type: 'single',
          message: {
            ...baseMessage,
            id: 'assistant-stream',
            content: [{ type: 'text', text: 'streaming output extended' }],
            isStreaming: true,
          },
          messages: [
            {
              ...baseMessage,
              id: 'assistant-stream',
              content: [{ type: 'text', text: 'streaming output extended' }],
              isStreaming: true,
            },
          ],
          coveredMessageIds: ['assistant-stream'],
        } as GroupedMessage,
      );
      rerender(<AgentChatMessages />);

      act(() => {
        while (frameQueue.length > 0) {
          frameQueue.shift()?.(0);
        }
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
      global.setTimeout = originalSetTimeout;
      global.clearTimeout = originalClearTimeout;
    }
  });

  it('keeps following bottom while busy tool results keep accumulating', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const frameQueue: FrameRequestCallback[] = [];

    chatState.messages = [
      {
        ...baseMessage,
        id: 'assistant-tool-call',
        content: [],
        tool_calls: [
          {
            id: 'call-1',
            type: 'function',
            function: {
              name: 'agent__runTool',
              arguments: '{}',
            },
          },
        ],
      },
      {
        id: 'tool-result-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call-1',
        content: [{ type: 'text', text: 'First tool result chunk' }],
      },
    ];
    chatState.workflowStatus = 'busy';
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      frameQueue.push(callback);
      return frameQueue.length;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();

    try {
      render(<AgentChatMessages />);
      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      const nextFrame = frameQueue.shift();
      expect(nextFrame).toBeDefined();

      act(() => {
        nextFrame?.(0);
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('re-aligns to the bottom when a session hydrates messages after mounting empty', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

    chatState.messages = [];
    groupedMessagesMock.splice(0, groupedMessagesMock.length);
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();

    try {
      const { rerender } = render(<AgentChatMessages />);
      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      chatState.messages = groupedToolMessages.slice(1);
      groupedMessagesMock.splice(
        0,
        groupedMessagesMock.length,
        {
          type: 'tool_group',
          message: baseMessage,
          messages: [baseMessage],
          coveredMessageIds: ['assistant-1', 'tool-1'],
          toolGroup: {
            calls: [
              {
                id: 'call-1',
                type: 'function',
                function: {
                  name: 'agent__compactSessionContext',
                  arguments: '{}',
                },
              },
            ],
            results: [],
          },
        },
      );

      rerender(<AgentChatMessages />);

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

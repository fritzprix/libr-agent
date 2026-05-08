import '@testing-library/jest-dom';
import { act, render, screen } from '@testing-library/react';
import React, { forwardRef } from 'react';
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

const { virtuosoMock, sessionState, chatState } = vi.hoisted(() => ({
  virtuosoMock: vi.fn(),
  sessionState: {
    session: { id: 'session-1', assistant: { name: 'Agent' } },
  },
  chatState: {
    messages: [] as Message[],
    workflowStatus: 'idle' as 'idle' | 'busy',
  },
}));

let resizeObserverCallback: ResizeObserverCallback | null = null;

class MockResizeObserver implements ResizeObserver {
  constructor(callback: ResizeObserverCallback) {
    resizeObserverCallback = callback;
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
    groupedMessages: groupedMessagesMock,
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
    void ref;
    return virtuosoMock(props);
  }),
}));

describe('AgentChatMessages compaction rendering', () => {
  beforeEach(() => {
    virtuosoMock.mockClear();
    resizeObserverCallback = null;
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
      initialTopMostItemIndex: number;
    };

    expect(virtuosoProps.initialTopMostItemIndex).toBe(
      virtuosoProps.firstItemIndex,
    );
    expect(getInitialTopMostItemIndex(10_000, 1)).toBe(10_000);
    expect(getInitialTopMostItemIndex(10_000, 3)).toBe(10_002);
  });

  it('keeps prepend index adjustments monotonic at zero instead of rebounding to list length', () => {
    expect(getPrependedFirstItemIndex(10_000, 3)).toBe(9_997);
    expect(getPrependedFirstItemIndex(2, 3)).toBe(0);
  });

  it('uses a tiny bottom threshold as scroll noise tolerance', () => {
    render(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      atBottomThreshold: number;
    };

    expect(virtuosoProps.atBottomThreshold).toBe(getVisualBottomThreshold());
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
        resizeObserverCallback?.([], {} as ResizeObserver);
      });

      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
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
      scrollIntoView.mockClear();

      sessionState.session = { id: 'session-2', assistant: { name: 'Agent' } };
      rerender(<AgentChatMessages />);

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
      scrollIntoView.mockClear();

      const nextFrame = frameQueue.shift();
      expect(nextFrame).toBeDefined();

      act(() => {
        nextFrame?.(0);
      });

      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
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
      scrollIntoView.mockClear();

      const nextFrame = frameQueue.shift();
      expect(nextFrame).toBeDefined();

      act(() => {
        nextFrame?.(0);
      });

      expect(scrollIntoView).toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

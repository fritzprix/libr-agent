import '@testing-library/jest-dom';
import { act, render } from '@testing-library/react';
import React, { forwardRef, useImperativeHandle } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentChatMessages } from '../AgentChatMessages';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import {
  baseMessage,
  groupedToolMessages,
  makeCompactToolGroupEntry,
  makeStreamingMessage,
  makeStreamingGroupEntry,
  applyVirtuosoMockImpl,
} from './AgentChatMessages.compaction-setup';

// ---------------------------------------------------------------------------
// Vitest hoisted shared state
// ---------------------------------------------------------------------------

const { virtuosoMock, scrollToIndexMock, sessionState, chatState, hasVirtuosoHandle } =
  vi.hoisted(() => ({
    virtuosoMock: vi.fn(),
    scrollToIndexMock: vi.fn(),
    sessionState: { session: { id: 'session-1', assistant: { name: 'Agent' } } },
    chatState: { messages: [] as Message[], workflowStatus: 'idle' as 'idle' | 'busy' },
    hasVirtuosoHandle: { current: true },
  }));

let groupedMessagesMock: GroupedMessage[] = [];
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

// ---------------------------------------------------------------------------
// Module mocks
// ---------------------------------------------------------------------------

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
  Tooltip: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  TooltipContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  TooltipProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('react-virtuoso', () => ({
  Virtuoso: forwardRef(function MockVirtuoso(props, ref) {
    useImperativeHandle(
      ref,
      () =>
        hasVirtuosoHandle.current
          ? { scrollToIndex: scrollToIndexMock }
          : (null as unknown as { scrollToIndex: typeof scrollToIndexMock }),
      [ref, hasVirtuosoHandle.current],
    );
    return virtuosoMock(props);
  }),
}));

// ---------------------------------------------------------------------------
// Per-test reset
// ---------------------------------------------------------------------------

beforeEach(() => {
  virtuosoMock.mockClear();
  scrollToIndexMock.mockClear();
  resizeObserverCallbacks = [];
  hasVirtuosoHandle.current = true;
  sessionState.session = { id: 'session-1', assistant: { name: 'Agent' } };
  chatState.messages = groupedToolMessages.slice(1);
  chatState.workflowStatus = 'idle';
  groupedMessagesMock.splice(0, groupedMessagesMock.length, makeCompactToolGroupEntry());
  applyVirtuosoMockImpl(virtuosoMock);
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AgentChatMessages – bottom alignment and retry', () => {
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
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

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
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

      expect(scrollToIndexMock.mock.calls.length).toBe(2);
      expect(scrollIntoView).not.toHaveBeenCalled();

      act(() => {
        virtuosoProps.atBottomStateChange?.(true);
        virtuosoProps.totalListHeightChanged?.(12_000);
      });

      expect(scrollToIndexMock.mock.calls.length).toBe(3);
      expect(scrollIntoView).not.toHaveBeenCalled();
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
      scrollToIndexMock.mockClear();
      scrollIntoView.mockClear();

      sessionState.session = { id: 'session-2', assistant: { name: 'Agent' } };
      rerender(<AgentChatMessages />);

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
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

    chatState.messages = [makeStreamingMessage()];
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
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

    chatState.messages = [makeStreamingMessage()];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(0, groupedMessagesMock.length, makeStreamingGroupEntry());

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
        makeStreamingGroupEntry(),
      );
      rerender(<AgentChatMessages />);

      const virtuosoPropsAfterPrepend = virtuosoMock.mock.lastCall?.[0] as {
        firstItemIndex: number;
      };

      expect(virtuosoPropsAfterPrepend.firstItemIndex).toBe(9_999);

      scrollToIndexMock.mockClear();

      chatState.messages = [makeStreamingMessage('streaming output extended')];
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
        makeStreamingGroupEntry('streaming output extended'),
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(scrollIntoView).not.toHaveBeenCalled();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

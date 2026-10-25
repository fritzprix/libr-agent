import '@testing-library/jest-dom';
import { act, render, screen } from '@testing-library/react';
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentChatMessages } from '../AgentChatMessages';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import {
  setScrollerMetrics,
  dispatchExplicitUpwardScroll,
  baseMessage,
  makeStreamingGroupEntry,
  makeStreamingMessage,
} from './AgentChatMessages.compaction-setup';
import { setupScrollFollowHarness } from './AgentChatMessages.scroll-follow.harness';

const {
  virtuosoMock,
  scrollToIndexMock,
  sessionState,
  chatState,
  hasVirtuosoHandle,
  groupedMessagesMock,
  resizeObserverCallbacks,
  resetHarness,
} = setupScrollFollowHarness();

const olderPageState = vi.hoisted(() => ({
  isLoadingOlderMessages: false,
  hasOlderMessages: true,
  loadOlderMessages: vi.fn(async () => undefined),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    messages: chatState.messages,
    pendingMessages: [],
    pendingQueue: [],
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
    hasOlderMessages: olderPageState.hasOlderMessages,
    isLoadingOlderMessages: olderPageState.isLoadingOlderMessages,
    loadOlderMessages: olderPageState.loadOlderMessages,
  }),
}));

vi.mock('@/context/LLMServiceContext', () => ({
  useLLMService: () => ({
    getCompactedRange: () => ({
      toId: 'tool-1',
      summary: 'Compacted summary',
    }),
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: { display: { messageLayout: 'bubble' } },
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

vi.mock('@/features/agent/components/shared', () => ({
  AnalysisLoader: () => <div>analysis loader</div>,
}));

vi.mock('../shared/CompactEventDivider', () => ({
  CompactEventDivider: ({ summary }: { summary?: string }) => (
    <div>{summary ?? 'compact divider'}</div>
  ),
}));

vi.mock('@/features/agent/components/PendingApprovalWidget', () => ({
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
  Virtuoso: React.forwardRef(function MockVirtuoso(props, ref) {
    React.useImperativeHandle(
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

beforeEach(() => {
  resetHarness();
  olderPageState.isLoadingOlderMessages = false;
  olderPageState.hasOlderMessages = true;
  olderPageState.loadOlderMessages.mockClear();
});

describe('AgentChatMessages – streaming follow', () => {
  it('keeps Non-Thinking stream height growth from yanking away from the top', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    const scrollIntoView = vi.fn();
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage('streaming output')];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      makeStreamingGroupEntry('streaming output'),
    );

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    performanceNowSpy.mockImplementation(() => currentTime);

    try {
      const { container, rerender } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();

      currentTime = 50;
      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 20,
      });
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 300;
      scroller!.scrollTop = 2;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
      scrollToIndexMock.mockClear();

      chatState.messages = [makeStreamingMessage('streaming output extended')];
      groupedMessagesMock.splice(
        0,
        groupedMessagesMock.length,
        makeStreamingGroupEntry('streaming output extended'),
      );
      rerender(<AgentChatMessages />);

      expect(scrollToIndexMock).not.toHaveBeenCalled();

      act(() => {
        resizeObserverCallbacks.current.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      expect(scrollToIndexMock).not.toHaveBeenCalled();
      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('ignores upward deltas inside the self-scroll ignore window', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    const scrollIntoView = vi.fn();
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage()];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(0, groupedMessagesMock.length, makeStreamingGroupEntry());

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
      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
      });

      currentTime = 50;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 388;

      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      scroller!.scrollTop = 376;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      scroller!.scrollTop = 364;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();
      expect(scrollToIndexMock).toHaveBeenCalled();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('keeps follow during streaming when resize-driven upward deltas have no user gesture', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    const scrollIntoView = vi.fn();
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage('streaming output')];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      makeStreamingGroupEntry('streaming output'),
    );

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    performanceNowSpy.mockImplementation(() => currentTime);

    try {
      const { container, rerender } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
      });

      currentTime = 300;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      chatState.messages = [makeStreamingMessage('streaming output extended')];
      groupedMessagesMock.splice(
        0,
        groupedMessagesMock.length,
        makeStreamingGroupEntry('streaming output extended'),
      );
      rerender(<AgentChatMessages />);

      act(() => {
        resizeObserverCallbacks.current.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      expect(scrollToIndexMock).toHaveBeenCalled();
      scrollToIndexMock.mockClear();

      scroller!.scrollTop = 388;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 376;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 364;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      act(() => {
        resizeObserverCallbacks.current.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      expect(scrollToIndexMock).toHaveBeenCalled();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });

  it('resets upward release accumulation on downward scrolls during prepend preservation', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const originalSetTimeout = global.setTimeout;
    const originalClearTimeout = global.clearTimeout;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    const timeoutQueue: Array<() => void> = [];
    const scrollIntoView = vi.fn();
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage()];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(0, groupedMessagesMock.length, makeStreamingGroupEntry());

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
    performanceNowSpy.mockImplementation(() => currentTime);

    try {
      const { container, rerender } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
      });

      currentTime = 50;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 250;
      scroller!.scrollTop = 370;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

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
          message: makeStreamingMessage(),
          messages: [makeStreamingMessage()],
          coveredMessageIds: ['assistant-stream'],
        } as GroupedMessage,
      );
      rerender(<AgentChatMessages />);

      currentTime = 300;
      scroller!.scrollTop = 380;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      act(() => {
        timeoutQueue.splice(0).forEach((callback) => callback());
      });

      currentTime = 500;
      act(() => {
        dispatchExplicitUpwardScroll(scroller!, 370);
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      currentTime = 700;
      act(() => {
        dispatchExplicitUpwardScroll(scroller!, 344);
      });

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
      global.setTimeout = originalSetTimeout;
      global.clearTimeout = originalClearTimeout;
    }
  });

  it('keeps a queued bottom-follow scroll despite upward noise inside the self-scroll window', () => {
    const scrollIntoView = vi.fn();
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    const frameQueue = new Map<number, FrameRequestCallback>();
    let nextFrameId = 1;

    const flushAllFrames = () => {
      while (frameQueue.size > 0) {
        const callbacks = [...frameQueue.values()];
        frameQueue.clear();
        callbacks.forEach((callback) => callback(0));
      }
    };

    global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      const frameId = nextFrameId++;
      frameQueue.set(frameId, callback);
      return frameId;
    }) as typeof requestAnimationFrame;
    global.cancelAnimationFrame = ((frameId: number) => {
      frameQueue.delete(frameId);
    }) as typeof cancelAnimationFrame;
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    try {
      const { container } = render(<AgentChatMessages />);
      const scroller = container.querySelector(
        '.agent-chat-scrollbar',
      ) as HTMLDivElement | null;

      expect(scroller).not.toBeNull();

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
      });

      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
        flushAllFrames();
      });

      scrollIntoView.mockClear();

      act(() => {
        resizeObserverCallbacks.current.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      expect(frameQueue.size).toBeGreaterThan(0);

      scroller!.scrollTop = 388;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 376;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      scroller!.scrollTop = 364;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      act(() => {
        flushAllFrames();
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();
    } finally {
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

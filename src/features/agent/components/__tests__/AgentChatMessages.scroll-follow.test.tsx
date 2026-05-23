import '@testing-library/jest-dom';
import { act, render, screen } from '@testing-library/react';
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

describe('AgentChatMessages – scroll-follow intent detection', () => {
  it('pauses bottom follow after three explicit upward scrolls', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage()];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(0, groupedMessagesMock.length, makeStreamingGroupEntry());

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

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
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

  it('resets upward release accumulation after a downward self-scroll returns toward bottom', () => {
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

      currentTime = 50;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 250;
      scroller!.scrollTop = 388;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 260;
      act(() => {
        resizeObserverCallbacks.forEach((callback) =>
          callback([], {} as ResizeObserver),
        );
      });

      currentTime = 300;
      scroller!.scrollTop = 400;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 500;
      scroller!.scrollTop = 388;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 700;
      scroller!.scrollTop = 376;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      currentTime = 900;
      scroller!.scrollTop = 364;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
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
      scroller!.scrollTop = 370;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      currentTime = 700;
      scroller!.scrollTop = 344;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
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

      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
        flushAllFrames();
      });

      scrollIntoView.mockClear();

      act(() => {
        resizeObserverCallbacks.forEach((callback) =>
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

  it('keeps bottom follow until three explicit upward scrolls leave the visual bottom', () => {
    const originalRequestAnimationFrame = global.requestAnimationFrame;
    const originalCancelAnimationFrame = global.cancelAnimationFrame;
    const performanceNowSpy = vi.spyOn(performance, 'now');
    let currentTime = 0;

    chatState.messages = [makeStreamingMessage()];
    chatState.workflowStatus = 'busy';
    groupedMessagesMock.splice(0, groupedMessagesMock.length, makeStreamingGroupEntry());

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

      expect(screen.getByLabelText('Scroll to latest')).toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
    }
  });

  it('resumes bottom follow when the user manually returns to latest during busy streaming', () => {
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

      expect(scrollToIndexMock).toHaveBeenCalled();
      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();
    } finally {
      performanceNowSpy.mockRestore();
      global.requestAnimationFrame = originalRequestAnimationFrame;
      global.cancelAnimationFrame = originalCancelAnimationFrame;
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

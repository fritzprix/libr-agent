import '@testing-library/jest-dom';
import { act, render, screen } from '@testing-library/react';
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentChatMessages } from '../AgentChatMessages';
import {
  setScrollerMetrics,
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
});

describe('AgentChatMessages – scroll intent detection', () => {
  it('pauses bottom follow via near-top threshold with a sub-36px upward delta', () => {
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

      currentTime = 50;
      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 20,
      });
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      expect(screen.queryByLabelText('Scroll to latest')).not.toBeInTheDocument();

      currentTime = 300;
      scroller!.scrollTop = 4;
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

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
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
      scroller!.scrollTop = 388;
      act(() => {
        scroller?.dispatchEvent(new Event('scroll'));
      });

      currentTime = 260;
      act(() => {
        resizeObserverCallbacks.current.forEach((callback) =>
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

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
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

      setScrollerMetrics(scroller!, {
        scrollHeight: 500,
        clientHeight: 100,
        scrollTop: 400,
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

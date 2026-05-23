import '@testing-library/jest-dom';
import { render } from '@testing-library/react';
import React, { forwardRef, useImperativeHandle } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AgentChatMessages,
  getInitialTopMostItemIndex,
  getGroupedMessageVirtuosoKey,
  getPrependedFirstItemIndex,
  isPinnedToBottom,
  getVisualBottomThreshold,
} from '../AgentChatMessages';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import {
  baseMessage,
  groupedToolMessages,
  makeCompactToolGroupEntry,
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

describe('AgentChatMessages – Virtuoso list configuration', () => {
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

  it('applies prepend firstItemIndex compensation in the same render as older message insertion', () => {
    const { rerender } = render(<AgentChatMessages />);

    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'prepended-older-1',
        },
        messages: [
          {
            ...baseMessage,
            id: 'prepended-older-1',
            role: 'user',
          },
        ],
        coveredMessageIds: ['prepended-older-1'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'prepended-older-2',
        },
        messages: [
          {
            ...baseMessage,
            id: 'prepended-older-2',
            role: 'assistant',
          },
        ],
        coveredMessageIds: ['prepended-older-2'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
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

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      firstItemIndex: number;
    };

    expect(virtuosoProps.firstItemIndex).toBe(9_998);
  });

  it('skips prepend compensation when the previous visible head is not preserved at the shifted offset', () => {
    const { rerender } = render(<AgentChatMessages />);

    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'new-head-group',
        },
        messages: [
          {
            ...baseMessage,
            id: 'new-head-group',
            role: 'assistant',
          },
        ],
        coveredMessageIds: ['new-head-group'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'merged-existing-group',
        },
        messages: [
          {
            ...baseMessage,
            id: 'merged-existing-group',
            role: 'assistant',
          },
        ],
        coveredMessageIds: ['merged-existing-group'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
    );

    rerender(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      firstItemIndex: number;
    };

    expect(virtuosoProps.firstItemIndex).toBe(10_000);
  });

  it('uses boundary membership to keep prepend compensation when the previous head is absorbed into a new group', () => {
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
      {
        type: 'single',
        message: {
          ...baseMessage,
          id: 'assistant-tail',
        },
        messages: [
          {
            ...baseMessage,
            id: 'assistant-tail',
          },
        ],
        coveredMessageIds: ['assistant-tail'],
      } as GroupedMessage,
    );

    const { rerender } = render(<AgentChatMessages />);

    groupedMessagesMock.splice(
      0,
      groupedMessagesMock.length,
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'prepended-older-group',
        },
        messages: [
          {
            ...baseMessage,
            id: 'prepended-older-group',
            role: 'assistant',
          },
        ],
        coveredMessageIds: ['prepended-older-group'],
        toolGroup: {
          calls: [],
          results: [],
        },
      },
      {
        type: 'tool_group',
        message: {
          ...baseMessage,
          id: 'merged-head-group',
        },
        messages: [
          {
            ...baseMessage,
            id: 'merged-head-group',
            role: 'assistant',
          },
          {
            ...baseMessage,
            id: 'assistant-1',
            role: 'assistant',
          },
        ],
        coveredMessageIds: ['merged-head-group', 'assistant-1', 'tool-1'],
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
      {
        type: 'single',
        message: {
          ...baseMessage,
          id: 'assistant-tail',
        },
        messages: [
          {
            ...baseMessage,
            id: 'assistant-tail',
          },
        ],
        coveredMessageIds: ['assistant-tail'],
      } as GroupedMessage,
    );

    rerender(<AgentChatMessages />);

    const virtuosoProps = virtuosoMock.mock.lastCall?.[0] as {
      firstItemIndex: number;
    };

    expect(virtuosoProps.firstItemIndex).toBe(9_999);
  });

  it('changes Virtuoso item keys when a grouped row changes structural shape', () => {
    const singleKey = getGroupedMessageVirtuosoKey({
      type: 'single',
      message: baseMessage,
      messages: [baseMessage],
      coveredMessageIds: ['assistant-1'],
    } as GroupedMessage);

    const toolGroupKey = getGroupedMessageVirtuosoKey({
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
    });

    const expandedToolGroupKey = getGroupedMessageVirtuosoKey({
      type: 'tool_group',
      message: baseMessage,
      messages: [
        baseMessage,
        {
          ...baseMessage,
          id: 'assistant-2',
        },
      ],
      coveredMessageIds: ['assistant-1', 'tool-1', 'assistant-2', 'tool-2'],
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
          {
            id: 'call-2',
            type: 'function',
            function: {
              name: 'agent__compactSessionContext',
              arguments: '{}',
            },
          },
        ],
        results: [],
      },
    });

    expect(singleKey).not.toBe(toolGroupKey);
    expect(toolGroupKey).not.toBe(expandedToolGroupKey);
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

  it('treats only tiny distances as pinned to the bottom', () => {
    expect(isPinnedToBottom(0, 50)).toBe(true);
    expect(isPinnedToBottom(4, 50)).toBe(true);
    expect(isPinnedToBottom(50, 50)).toBe(true);
    expect(isPinnedToBottom(51, 50)).toBe(false);
  });
});

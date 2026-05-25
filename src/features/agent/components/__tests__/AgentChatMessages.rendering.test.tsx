import '@testing-library/jest-dom';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentChatMessages, shouldShowAnalysisLoader } from '../AgentChatMessages';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import {
  installMockResizeObserver,
  resetAgentChatMessagesHarness,
  baseMessage,
  type AgentChatMessagesTestHarness,
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
const resizeObserverCallbacks = { current: [] as ResizeObserverCallback[] };

installMockResizeObserver(resizeObserverCallbacks);

// ---------------------------------------------------------------------------
// Module mocks
// ---------------------------------------------------------------------------

const harness: AgentChatMessagesTestHarness = {
  virtuosoMock,
  scrollToIndexMock,
  sessionState,
  chatState,
  hasVirtuosoHandle,
};

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

// ---------------------------------------------------------------------------
// Per-test reset
// ---------------------------------------------------------------------------

beforeEach(() => {
  resetAgentChatMessagesHarness({
    harness,
    groupedMessagesMock,
    resizeObserverCallbacks,
  });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AgentChatMessages – rendering and layout helpers', () => {
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
});

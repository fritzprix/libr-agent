import '@testing-library/jest-dom';
import { render, screen } from '@testing-library/react';
import { forwardRef } from 'react';
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

const { virtuosoMock } = vi.hoisted(() => ({
  virtuosoMock: vi.fn(),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    messages: groupedToolMessages.slice(1),
    pendingMessages: [],
    error: undefined,
    llmError: undefined,
    retryMessage: vi.fn(),
    workflowStatus: 'idle',
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSession: () => ({
    session: { id: 'session-1', assistant: { name: 'Agent' } },
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

vi.mock('react-virtuoso', () => ({
  Virtuoso: forwardRef(function MockVirtuoso(props, ref) {
    void ref;
    return virtuosoMock(props);
  }),
}));

describe('AgentChatMessages compaction rendering', () => {
  beforeEach(() => {
    virtuosoMock.mockClear();
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
        };
        context: unknown;
        data: GroupedMessage[];
        itemContent: (
          index: number,
          item: GroupedMessage,
        ) => JSX.Element | null;
      }) => (
        <div>
          {components?.Header ? <components.Header context={context} /> : null}
          {data.map((item, index) => (
            <div key={item.message.id}>{itemContent(index, item)}</div>
          ))}
          {components?.Footer ? <components.Footer context={context} /> : null}
        </div>
      ),
    );
  });

  it('renders the compact event when the boundary falls inside a tool group', () => {
    render(<AgentChatMessages />);

    expect(screen.getByText('Compacted summary')).toBeInTheDocument();
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

  it('treats only tiny distances as pinned to the bottom', () => {
    expect(isPinnedToBottom(0)).toBe(true);
    expect(isPinnedToBottom(4)).toBe(true);
    expect(isPinnedToBottom(5)).toBe(false);
  });
});

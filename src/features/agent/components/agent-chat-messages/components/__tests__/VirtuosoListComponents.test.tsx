import '@testing-library/jest-dom';
import { createRef } from 'react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  AgentChatMessagesHeader,
  AgentChatMessagesList,
} from '../VirtuosoListComponents';
import {
  CHAT_LIST_HEADER_MIN_HEIGHT_PX,
  type AgentChatVirtuosoContext,
} from '../../types';

function createContext(
  overrides: Partial<AgentChatVirtuosoContext> = {},
): AgentChatVirtuosoContext {
  return {
    agentError: null,
    agentLlmError: null,
    footerEndRef: createRef<HTMLDivElement | null>(),
    hasOlderMessages: false,
    isLoadingOlderMessages: false,
    latestMessage: undefined,
    loadingOlderLabel: 'Loading older messages...',
    pendingApprovals: [],
    respondToToolApproval: vi.fn(),
    retryMessage: vi.fn(),
    scrollToLoadOlderLabel: 'Scroll up to load older messages',
    sessionAssistantName: 'Agent',
    workflowStatus: 'idle',
    executionMode: 'normal',
    messageLayout: 'bubble',
    ...overrides,
  };
}

describe('VirtuosoListComponents', () => {
  it('keeps a fixed header height when there are no older messages', () => {
    render(<AgentChatMessagesHeader context={createContext()} />);

    const header = screen.getByTestId('agent-chat-messages-header');
    expect(header).toHaveAttribute('aria-hidden', 'true');
    expect(header).toHaveStyle({
      height: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
      minHeight: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
    });
    expect(
      screen.queryByText('Scroll up to load older messages'),
    ).not.toBeInTheDocument();
  });

  it('reuses the same fixed header height when showing the load-older pill', () => {
    render(
      <AgentChatMessagesHeader
        context={createContext({ hasOlderMessages: true })}
      />,
    );

    const header = screen.getByTestId('agent-chat-messages-header');
    expect(header).toHaveAttribute('aria-hidden', 'false');
    expect(header).toHaveStyle({
      height: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
      minHeight: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
    });
    expect(
      screen.getByText('Scroll up to load older messages'),
    ).toBeInTheDocument();
  });

  it('keeps the same fixed header height while the older-page loading pill is shown', () => {
    render(
      <AgentChatMessagesHeader
        context={createContext({
          hasOlderMessages: true,
          isLoadingOlderMessages: true,
        })}
      />,
    );

    const header = screen.getByTestId('agent-chat-messages-header');
    expect(header).toHaveStyle({
      height: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
      minHeight: `${CHAT_LIST_HEADER_MIN_HEIGHT_PX}px`,
    });
    expect(screen.getByText('Loading older messages...')).toBeInTheDocument();
  });

  it('preserves Virtuoso List paddingTop while adding horizontal padding', () => {
    const { container } = render(
      <AgentChatMessagesList
        context={createContext()}
        style={{ paddingTop: '123px', paddingBottom: '45px' }}
        data-testid="agent-chat-messages-list"
      >
        <div>item</div>
      </AgentChatMessagesList>,
    );

    const list = container.querySelector(
      '[data-testid="agent-chat-messages-list"]',
    ) as HTMLDivElement | null;

    expect(list).not.toBeNull();
    expect(list).toHaveStyle({
      paddingTop: '123px',
      paddingBottom: '45px',
      paddingLeft: '16px',
      paddingRight: '16px',
    });
  });

  it('uses wider horizontal padding in document message layout', () => {
    const { container } = render(
      <AgentChatMessagesList
        context={createContext({ messageLayout: 'document' })}
        style={{ paddingTop: '10px' }}
        data-testid="agent-chat-messages-list-document"
      >
        <div>item</div>
      </AgentChatMessagesList>,
    );

    const list = container.querySelector(
      '[data-testid="agent-chat-messages-list-document"]',
    ) as HTMLDivElement | null;

    expect(list).not.toBeNull();
    expect(list).toHaveStyle({
      paddingLeft: '24px',
      paddingRight: '24px',
    });
  });
});

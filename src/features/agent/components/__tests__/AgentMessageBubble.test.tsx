import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentMessageBubble } from '../AgentMessageBubble';
import type { Message } from '@/models/chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback || key,
    i18n: { language: 'en' },
  }),
}));

vi.mock('../AgentMessageRenderer', () => ({
  AgentMessageRenderer: ({ content }: { content: unknown }) => (
    <div data-testid="agent-message-renderer">{JSON.stringify(content)}</div>
  ),
}));

vi.mock('../MessageActionBar', () => ({
  MessageActionBar: () => <div data-testid="message-action-bar" />,
}));

const createMessage = (overrides: Partial<Message> = {}): Message => ({
  id: 'msg-1',
  sessionId: 'session-1',
  threadId: 'session-1',
  role: 'tool',
  content: [{ type: 'text', text: 'Error: file not found' }],
  createdAt: new Date(),
  ...overrides,
});

describe('AgentMessageBubble - Collapsible Tool Error Group', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  describe('Simple mode (toolDetailLevel === "simple")', () => {
    it('is collapsed by default for toolErrorGroup', () => {
      const msg = createMessage();

      render(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      const header = screen.getByRole('button');
      expect(header).toBeInTheDocument();
      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.getByText('Tool Execution Error')).toBeInTheDocument();
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();
    });

    it('displays error count when multiple messages are grouped', () => {
      const msg1 = createMessage({ id: 'msg-1' });
      const msg2 = createMessage({ id: 'msg-2' });
      const msg3 = createMessage({ id: 'msg-3' });

      render(
        <AgentMessageBubble
          message={msg1}
          groupedMessages={[msg1, msg2, msg3]}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      expect(screen.getByText('(3)')).toBeInTheDocument();
    });

    it('does not display count when only 1 message is in groupedMessages', () => {
      const msg = createMessage();

      render(
        <AgentMessageBubble
          message={msg}
          groupedMessages={[msg]}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      expect(screen.queryByText(/\(1\)/)).not.toBeInTheDocument();
    });

    it('expands and collapses upon clicking the header', () => {
      const msg = createMessage();

      render(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      const header = screen.getByRole('button');
      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();

      // Click to expand
      fireEvent.click(header);
      expect(header).toHaveAttribute('aria-expanded', 'true');
      expect(screen.getByTestId('agent-message-renderer')).toBeInTheDocument();

      // Click to collapse again
      fireEvent.click(header);
      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();
    });

    it('follows live toolDetailLevel prop changes when user has not manually toggled', () => {
      const msg = createMessage();

      const { rerender } = render(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      const header = screen.getByRole('button');
      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();

      // Switch to developer mode
      rerender(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="developer"
        />,
      );

      expect(header).toHaveAttribute('aria-expanded', 'true');
      expect(screen.getByTestId('agent-message-renderer')).toBeInTheDocument();

      // Switch back to simple mode
      rerender(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();
    });
  });

  describe('Developer mode (toolDetailLevel === "developer")', () => {
    it('is expanded by default for toolErrorGroup', () => {
      const msg = createMessage();

      render(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={true}
          messageLayout="document"
          toolDetailLevel="developer"
        />,
      );

      const header = screen.getByRole('button');
      expect(header).toHaveAttribute('aria-expanded', 'true');
      expect(screen.getByTestId('agent-message-renderer')).toBeInTheDocument();

      // Can be collapsed by user
      fireEvent.click(header);
      expect(header).toHaveAttribute('aria-expanded', 'false');
      expect(screen.queryByTestId('agent-message-renderer')).not.toBeInTheDocument();
    });
  });

  describe('Normal messages (toolErrorGroup === false)', () => {
    it('renders normal header and content without toolError collapsible header', () => {
      const msg = createMessage({ role: 'assistant', content: [{ type: 'text', text: 'Hello' }] });

      render(
        <AgentMessageBubble
          message={msg}
          toolErrorGroup={false}
          messageLayout="document"
          toolDetailLevel="simple"
        />,
      );

      expect(screen.queryByText('Tool Execution Error')).not.toBeInTheDocument();
      expect(screen.getByTestId('agent-message-renderer')).toBeInTheDocument();
    });
  });
});

import { render, screen, cleanup } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentMessageBubble } from '../AgentMessageBubble';
import type { Message } from '@/models/chat';

// Interpolates `{{name}}` in `defaultValue` so the notice text is checkable.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: string | Record<string, unknown>) => {
      if (typeof opts === 'string') {
        return opts;
      }
      if (opts && typeof opts.defaultValue === 'string') {
        return Object.entries(opts).reduce(
          (text, [name, value]) =>
            text.replaceAll(`{{${name}}}`, String(value)),
          opts.defaultValue,
        );
      }
      return key;
    },
    i18n: { language: 'en' },
  }),
}));

vi.mock('../AgentMessageRenderer', () => ({
  AgentMessageRenderer: () => (
    <div data-testid="agent-message-renderer" />
  ),
}));

vi.mock('../MessageActionBar', () => ({
  MessageActionBar: () => <div data-testid="message-action-bar" />,
}));

const createNoticeMessage = (): Message => ({
  id: 'notice-1',
  sessionId: 'session-1',
  threadId: 'session-1',
  role: 'system',
  content: [
    {
      type: 'text',
      text: 'The server rejected the request (HTTP 400). Retrying with fallback model gpt-4o-mini.',
    },
  ],
  createdAt: new Date(),
  metadata: {
    fallbackNotice: {
      statusCode: 400,
      fallbackModel: 'gpt-4o-mini',
      errorMessage:
        'LLM provider returned HTTP 400: context length exceeded',
    },
  },
});

describe('AgentMessageBubble - fallback model notice (#2961)', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('shows the HTTP status and the fallback model name', () => {
    render(<AgentMessageBubble message={createNoticeMessage()} />);

    const notice = screen.getByTestId('llm-fallback-notice');
    expect(notice).toBeInTheDocument();
    expect(notice.textContent).toContain('HTTP 400');
    expect(notice.textContent).toContain('gpt-4o-mini');
  });

  it('keeps the raw server error in an expandable detail', () => {
    render(<AgentMessageBubble message={createNoticeMessage()} />);

    const notice = screen.getByTestId('llm-fallback-notice');
    const details = notice.querySelector('details');
    expect(details).not.toBeNull();
    expect(details?.textContent).toContain(
      'LLM provider returned HTTP 400: context length exceeded',
    );
  });

  it('does not replace ordinary messages with the notice', () => {
    render(
      <AgentMessageBubble
        message={{
          ...createNoticeMessage(),
          metadata: undefined,
        }}
      />,
    );

    expect(screen.queryByTestId('llm-fallback-notice')).not.toBeInTheDocument();
    expect(screen.getByTestId('agent-message-renderer')).toBeInTheDocument();
  });
});

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import type { Message } from '@/models/chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'test-session', assistant: { id: 'test-assistant' } },
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: vi.fn(),
    injectMessages: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      toolCallGroupVisibleCount: 4,
    },
    update: vi.fn(),
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: vi.fn(),
  }),
}));

vi.mock('next-themes', () => ({
  useTheme: () => ({
    resolvedTheme: 'dark',
  }),
}));

vi.mock('@/hooks/use-is-dark-mode', () => ({
  useIsDarkMode: () => true,
}));

describe('AgentMessageRenderer prefill indicator', () => {
  it('renders prefill indicator when content is empty and streamingPhase is prefill', () => {
    const prefillMsg: Message = {
      id: 'msg-prefill',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'assistant',
      content: [],
      isStreaming: true,
      streamingPhase: 'prefill',
    };

    render(<AgentMessageRenderer message={prefillMsg} />);

    const indicator = screen.getByTestId('streaming-prefill-indicator');
    expect(indicator).toBeInTheDocument();
    expect(indicator).toHaveTextContent('Preparing prompt...');
  });

  it('does not render prefill indicator when message is not streaming', () => {
    const nonStreamingMsg: Message = {
      id: 'msg-idle',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'assistant',
      content: [],
      isStreaming: false,
    };

    const { container } = render(
      <AgentMessageRenderer message={nonStreamingMsg} />,
    );

    expect(
      screen.queryByTestId('streaming-prefill-indicator'),
    ).not.toBeInTheDocument();
    expect(container.firstChild).toBeNull();
  });
});

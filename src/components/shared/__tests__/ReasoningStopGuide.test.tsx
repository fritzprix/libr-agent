import '@testing-library/jest-dom';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MessageError } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';

const mockNavigate = vi.fn();

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

// Returns the English fallback with `{{name}}` placeholders filled in.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      _key: string,
      fallback?: string,
      values?: Record<string, string | number>,
    ) =>
      (fallback ?? '').replace(/\{\{(\w+)\}\}/g, (_match, name: string) =>
        String(values?.[name] ?? ''),
      ),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({ info: vi.fn() }),
}));

function stopError(errorCode: string, originalError: unknown): MessageError {
  return {
    // The Rust message; the guide replaces it.
    displayMessage: 'Stopped: raw backend message. Click Retry to try again.',
    type: 'AI_SERVICE_ERROR',
    recoverable: true,
    details: {
      originalError,
      errorCode,
      timestamp: '2026-11-12T12:00:00.000Z',
    },
  };
}

describe('ErrorBubble reasoning-stop guide', () => {
  beforeEach(() => {
    mockNavigate.mockClear();
  });

  it('REASONING_BUDGET_EXCEEDED shows the numbers, the Settings source, and an Open Settings button', () => {
    render(
      <ErrorBubble
        error={stopError('REASONING_BUDGET_EXCEEDED', {
          reasoningBudgetTokens: 7372,
          estimatedThinkingTokens: 7400,
          maxOutputTokens: 8192,
          maxOutputTokensSource: 'settings',
          maxRetries: 1,
        })}
      />,
    );

    expect(
      screen.getByText(
        `Used about ${(7400).toLocaleString()} tokens. The cutoff is ${(7372).toLocaleString()} tokens, which is 90% of Max Output Tokens.`,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        `Max Output Tokens is ${(8192).toLocaleString()}, from Settings → Chat Interface → Max Output Tokens (Default).`,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText('Automatic retries already used: 1.'),
    ).toBeInTheDocument();
    expect(screen.queryByText(/raw backend message/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Max Input Context/)).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: 'Open Max Output Tokens Settings' }),
    );
    expect(mockNavigate).toHaveBeenCalledWith('/settings?tab=chat-interface');
    expect(
      screen.getByRole('button', { name: 'Try Again' }),
    ).toBeInTheDocument();
  });

  it('REASONING_BUDGET_EXCEEDED with an assistant-level value does not send the user to Settings', () => {
    render(
      <ErrorBubble
        error={stopError('REASONING_BUDGET_EXCEEDED', {
          reasoningBudgetTokens: 3686,
          estimatedThinkingTokens: 3700,
          maxOutputTokens: 4096,
          maxOutputTokensSource: 'assistant',
        })}
      />,
    );

    expect(
      screen.getByText(
        `Max Output Tokens is ${(4096).toLocaleString()}, set on this assistant. Changing the Settings default will not change this session.`,
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Open Max Output Tokens Settings' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/raise Max Output Tokens/)).not.toBeInTheDocument();
  });

  it('REASONING_BUDGET_EXCEEDED without numbers still explains the stop', () => {
    render(<ErrorBubble error={stopError('REASONING_BUDGET_EXCEEDED', {})} />);

    expect(
      screen.getByText(/spent its output budget on reasoning/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Used about/)).not.toBeInTheDocument();
  });

  it('REPEATED_THINKING_LOOP shows the pattern numbers and points to the Advanced loop settings', () => {
    render(
      <ErrorBubble
        error={stopError('REPEATED_THINKING_LOOP', {
          patternLength: 256,
          repetitionCount: 4,
          maxRetries: 2,
        })}
      />,
    );

    expect(
      screen.getByText('Detected a 256-character pattern repeated 4 times.'),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Thinking Loop Min Pattern Length/),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: 'Open Advanced Settings' }),
    );
    expect(mockNavigate).toHaveBeenCalledWith('/settings?tab=advanced');
  });

  it('REPEATED_TEXT_LOOP does not point to the thinking-loop settings, which do not apply to it', () => {
    render(
      <ErrorBubble
        error={stopError('REPEATED_TEXT_LOOP', {
          patternLength: 64,
          repetitionCount: 3,
        })}
      />,
    );

    expect(
      screen.getByText(/kept repeating the same text/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Thinking Loop Min/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Open Advanced Settings' }),
    ).not.toBeInTheDocument();
  });

  it('THINKING_ONLY_COMPLETION explains the stop and offers the output-token setting', () => {
    render(
      <ErrorBubble
        error={stopError('THINKING_ONLY_COMPLETION', { maxRetries: 2 })}
      />,
    );

    expect(
      screen.getByText(
        'The model finished with only internal reasoning: no answer and no tool call.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/ask for a direct answer/)).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Open Max Output Tokens Settings' }),
    ).toBeInTheDocument();
  });

  it('other error codes keep the plain message', () => {
    render(<ErrorBubble error={stopError('SOMETHING_ELSE', {})} />);

    expect(screen.getByText(/raw backend message/)).toBeInTheDocument();
  });
});

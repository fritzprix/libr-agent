import '@testing-library/jest-dom';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MessageError } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';

const mockNavigate = vi.fn();

const loggerMocks = vi.hoisted(() => ({
  info: vi.fn(),
}));

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? '',
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => loggerMocks,
}));

describe('ErrorBubble', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockNavigate.mockClear();
  });

  it('logs the same error only once across rerenders', () => {
    const error: MessageError = {
      displayMessage: 'compact() received an empty response from streamChat',
      type: 'AI_SERVICE_ERROR',
      recoverable: true,
      details: {
        originalError: new Error('empty response'),
        timestamp: '2026-05-30T03:13:40.756Z',
      },
    };

    const { rerender } = render(<ErrorBubble error={error} />);

    rerender(<ErrorBubble error={error} />);
    rerender(<ErrorBubble error={{ ...error }} />);

    expect(loggerMocks.info).toHaveBeenCalledTimes(1);
    expect(loggerMocks.info).toHaveBeenCalledWith('Rendering error bubble', {
      error,
    });
  });

  it('logs a new error event when the error timestamp changes', () => {
    const firstOriginalError = new Error('empty response');
    const firstError: MessageError = {
      displayMessage: 'compact() received an empty response from streamChat',
      type: 'AI_SERVICE_ERROR',
      recoverable: true,
      details: {
        originalError: firstOriginalError,
        timestamp: '2026-05-30T03:13:40.756Z',
      },
    };

    const secondError: MessageError = {
      ...firstError,
      details: {
        originalError: firstOriginalError,
        timestamp: '2026-05-30T03:14:41.001Z',
      },
    };

    const { rerender } = render(<ErrorBubble error={firstError} />);

    rerender(<ErrorBubble error={secondError} />);

    expect(loggerMocks.info).toHaveBeenCalledTimes(2);
  });

  it('displays the "Go to Settings" button for AUTHENTICATION_ERROR and clicking navigates to /settings', () => {
    const error: MessageError = {
      displayMessage: 'Invalid API Key provided',
      type: 'AUTHENTICATION_ERROR',
      recoverable: false,
    };

    render(<ErrorBubble error={error} />);

    const settingsButton = screen.getByRole('button', {
      name: /Configure API Key in Settings/i,
    });
    expect(settingsButton).toBeInTheDocument();

    fireEvent.click(settingsButton);
    expect(mockNavigate).toHaveBeenCalledWith('/settings?tab=ai-models');
  });

  it('guides INVALID_CONTEXT_STATE to chat-interface context settings', () => {
    const error: MessageError = {
      displayMessage:
        'Prepared payload exceeds the effective context limit, but there is no ownership-safe compaction split that can reduce the live prompt. This session state is invalid and must not be committed.',
      type: 'CONTEXT_LIMIT_ERROR',
      recoverable: true,
      details: {
        originalError: {},
        errorCode: 'INVALID_CONTEXT_STATE',
        timestamp: '2026-10-08T11:35:39.894Z',
      },
    };

    render(<ErrorBubble error={error} />);

    expect(
      screen.getByText(
        /This session needs a larger context limit\. Open Settings, raise Max Input Context, then try again\./i,
      ),
    ).toBeInTheDocument();

    const settingsButton = screen.getByRole('button', {
      name: /Open Context Settings/i,
    });
    fireEvent.click(settingsButton);
    expect(mockNavigate).toHaveBeenCalledWith(
      '/settings?tab=chat-interface',
    );
  });
});

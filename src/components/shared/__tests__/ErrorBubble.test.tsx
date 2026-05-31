import '@testing-library/jest-dom';
import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MessageError } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';

const loggerMocks = vi.hoisted(() => ({
  info: vi.fn(),
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
});

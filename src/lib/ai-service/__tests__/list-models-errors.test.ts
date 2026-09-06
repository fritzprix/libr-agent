import { beforeEach, describe, expect, it, vi } from 'vitest';

const toastError = vi.fn();

vi.mock('sonner', () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
  },
}));

import {
  describeListModelsFailure,
  listModelsFailureToastMessage,
  reportListModelsFallback,
  subscribeListModelsFallback,
} from '../list-models-errors';

describe('list-models-errors', () => {
  beforeEach(() => {
    toastError.mockClear();
  });

  it('describeListModelsFailure builds a structured payload', () => {
    const payload = describeListModelsFailure({
      provider: 'openai',
      baseUrl: 'https://integrate.api.nvidia.com/v1',
      reason: 'api_error',
      error: new Error('Connection error.'),
    });

    expect(payload).toEqual({
      reason: 'api_error',
      provider: 'openai',
      baseUrl: 'https://integrate.api.nvidia.com/v1',
      message: 'Connection error.',
      usedStaticFallback: true,
      notifyUser: false,
    });
  });

  it('listModelsFailureToastMessage includes provider and detail', () => {
    expect(
      listModelsFailureToastMessage('openai', new Error('Connection error.')),
    ).toBe('Failed to fetch models for openai: Connection error.');
  });

  it('reportListModelsFallback stays silent by default and notifies subscribers', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeListModelsFallback(listener);

    const payload = reportListModelsFallback({
      provider: 'openai',
      reason: 'api_error',
      error: new Error('Connection error.'),
    });

    expect(listener).toHaveBeenCalledWith(payload);
    expect(payload.notifyUser).toBe(false);
    expect(toastError).not.toHaveBeenCalled();

    unsubscribe();
  });

  it('reportListModelsFallback toasts only when notifyUser is true', () => {
    const payload = reportListModelsFallback({
      provider: 'openai',
      reason: 'api_error',
      error: new Error('Connection error.'),
      notifyUser: true,
    });

    expect(payload.notifyUser).toBe(true);
    expect(toastError).toHaveBeenCalledWith(
      'Failed to fetch models for openai: Connection error.',
    );
  });

  it('skips toast when notifyUser is true but cached models exist', () => {
    reportListModelsFallback({
      provider: 'openai',
      reason: 'api_error',
      error: new Error('Connection error.'),
      notifyUser: true,
      hasCachedModels: true,
    });

    expect(toastError).not.toHaveBeenCalled();
  });

  it('deduplicates identical notifyUser toasts within 5 seconds', () => {
    vi.useFakeTimers();
    // Far from wall-clock so prior tests' lastToastAt cannot collide.
    vi.setSystemTime(new Date('2099-01-01T00:00:00.000Z'));

    const error = new Error('Dedup probe error');
    reportListModelsFallback({
      provider: 'dedup-provider',
      reason: 'api_error',
      error,
      notifyUser: true,
    });
    reportListModelsFallback({
      provider: 'dedup-provider',
      reason: 'api_error',
      error,
      notifyUser: true,
    });

    expect(toastError).toHaveBeenCalledTimes(1);

    vi.setSystemTime(new Date('2099-01-01T00:00:06.000Z'));
    reportListModelsFallback({
      provider: 'dedup-provider',
      reason: 'api_error',
      error,
      notifyUser: true,
    });

    expect(toastError).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
});

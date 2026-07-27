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
    });
  });

  it('listModelsFailureToastMessage includes provider and detail', () => {
    expect(
      listModelsFailureToastMessage('openai', new Error('Connection error.')),
    ).toBe('Failed to fetch models for openai: Connection error.');
  });

  it('reportListModelsFallback toasts and notifies subscribers', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeListModelsFallback(listener);

    const payload = reportListModelsFallback({
      provider: 'openai',
      reason: 'api_error',
      error: new Error('Connection error.'),
    });

    expect(listener).toHaveBeenCalledWith(payload);
    expect(toastError).toHaveBeenCalledWith(
      'Failed to fetch models for openai: Connection error.',
    );

    unsubscribe();
  });
});

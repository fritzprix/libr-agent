import { describe, expect, it, vi } from 'vitest';
import {
  throwStreamingError,
  withRetryPolicy,
} from '../base-service-strategies';
import type { StreamingErrorContext } from '../base-service-shared';
import { AIServiceError, AIServiceProvider } from '../types';

const context: StreamingErrorContext = {
  messages: [],
  options: {
    modelName: 'gemini-2.5-flash',
  },
  config: {},
};

describe('withRetryPolicy', () => {
  it('honors maxRetries from config', async () => {
    const logger = { warn: vi.fn() };
    const operation = vi
      .fn()
      .mockRejectedValueOnce(new Error('temporary'))
      .mockRejectedValueOnce(new Error('temporary'))
      .mockResolvedValueOnce('ok');

    const result = await withRetryPolicy({
      fn: operation,
      config: { maxRetries: 2, retryDelay: 1 },
      logger,
      provider: AIServiceProvider.OpenAI,
      shouldRetry: () => true,
    });

    expect(result).toBe('ok');
    expect(operation).toHaveBeenCalledTimes(3);
  });

  it('does not retry beyond maxRetries', async () => {
    const logger = { warn: vi.fn() };
    const operation = vi.fn().mockRejectedValue(new Error('always fails'));

    await expect(
      withRetryPolicy({
        fn: operation,
        config: { maxRetries: 1, retryDelay: 1 },
        logger,
        provider: AIServiceProvider.OpenAI,
        shouldRetry: () => true,
      }),
    ).rejects.toThrow('always fails');

    expect(operation).toHaveBeenCalledTimes(2);
  });
});

describe('throwStreamingError', () => {
  it('classifies RESOURCE_EXHAUSTED provider statuses as rate limits', () => {
    const logger = {
      info: vi.fn(),
      error: vi.fn(),
    };

    try {
      throwStreamingError({
        error: {
          error: {
            status: 'RESOURCE_EXHAUSTED',
            message: 'Rate limit exceeded. Please retry later.',
          },
        },
        context,
        logger,
        provider: AIServiceProvider.Gemini,
      });
    } catch (error) {
      expect(error).toBeInstanceOf(AIServiceError);
      if (!(error instanceof AIServiceError)) {
        throw error;
      }

      expect(error.metadata.kind).toBe('rate_limit');
      expect(error.metadata.retryable).toBe(true);
      expect(error.metadata.providerStatus).toBe('RESOURCE_EXHAUSTED');
      return;
    }

    throw new Error('Expected throwStreamingError to throw');
  });

  it('classifies prompt-too-long 400s as context-limit errors', () => {
    const logger = {
      info: vi.fn(),
      error: vi.fn(),
    };

    try {
      throwStreamingError({
        error: {
          status: 400,
          error: {
            message:
              '400 Prompt too long: 146228 tokens exceeds max context window of 131072 tokens',
          },
        },
        context,
        logger,
        provider: AIServiceProvider.OpenAI,
      });
    } catch (error) {
      expect(error).toBeInstanceOf(AIServiceError);
      if (!(error instanceof AIServiceError)) {
        throw error;
      }

      expect(error.metadata.kind).toBe('context_limit');
      expect(error.metadata.retryable).toBe(false);
      expect(error.statusCode).toBe(400);
      return;
    }

    throw new Error('Expected throwStreamingError to throw');
  });

  it('classifies prefill memory overflow as a context-limit error', () => {
    const logger = {
      info: vi.fn(),
      error: vi.fn(),
    };

    try {
      throwStreamingError({
        error: {
          error: {
            type: 'server_error',
            message:
              'Prefill context too large for available memory (pre-chunk guard at 2048 tokens, kv_len=81920): predicted peak would exceed prefill safety cap 46.7GB (90% of effective ceiling 51.8GB)',
          },
        },
        context,
        logger,
        provider: AIServiceProvider.OpenAI,
      });
    } catch (error) {
      expect(error).toBeInstanceOf(AIServiceError);
      if (!(error instanceof AIServiceError)) {
        throw error;
      }

      expect(error.metadata.kind).toBe('context_limit');
      expect(error.metadata.retryable).toBe(false);
      expect(error.metadata.providerStatus).toBe('server_error');
      return;
    }

    throw new Error('Expected throwStreamingError to throw');
  });
});

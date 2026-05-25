import { describe, expect, it, vi } from 'vitest';
import { throwStreamingError } from '../base-service-strategies';
import type { StreamingErrorContext } from '../base-service-shared';
import { AIServiceError, AIServiceProvider } from '../types';

const context: StreamingErrorContext = {
  messages: [],
  options: {
    modelName: 'gemini-2.5-flash',
  },
  config: {},
};

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
});

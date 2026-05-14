import { describe, expect, it } from 'vitest';
import { AIServiceError, AIServiceProvider } from '@/lib/ai-service/types';
import {
  shouldBypassRetryAndFallback,
  toAgentRuntimeError,
} from '../listener-utils';

describe('listener-utils overflow recovery helpers', () => {
  it('converts typed provider overflow into CONTEXT_LIMIT_ERROR', () => {
    const error = new AIServiceError(
      'openai streaming failed: Context size has been exceeded.',
      AIServiceProvider.OpenAI,
      500,
      undefined,
      {
        kind: 'context_limit',
        retryable: false,
        providerStatus: 'server_error',
        providerCode: 500,
      },
    );

    expect(toAgentRuntimeError(error)).toEqual(
      expect.objectContaining({
        type: 'CONTEXT_LIMIT_ERROR',
        displayMessage: 'Context size has been exceeded.',
        recoverable: true,
        details: expect.objectContaining({
          errorCode: 'CONTEXT_LIMIT_EXCEEDED',
        }),
      }),
    );
  });

  it('bypasses retry and fallback for typed provider overflow', () => {
    const error = new AIServiceError(
      'openai streaming failed: Context size has been exceeded.',
      AIServiceProvider.OpenAI,
      500,
      undefined,
      {
        kind: 'context_limit',
        retryable: false,
      },
    );

    expect(shouldBypassRetryAndFallback(error)).toBe(true);
  });
});

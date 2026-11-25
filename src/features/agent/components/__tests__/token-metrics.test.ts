import { describe, expect, it } from 'vitest';

import type { TokenUsage } from '@/lib/ai-service/types';

import {
  calculateCacheHitPercent,
  mergeDisplayTokenUsage,
} from '../token-metrics';

describe('token-metrics helpers', () => {
  it('clamps cache hit percent to 100', () => {
    expect(calculateCacheHitPercent(24148, 8389)).toBe(100);
  });

  it('does not carry stale cached token fields or durations into a new turn', () => {
    const previous: TokenUsage = {
      promptTokens: 24148,
      completionTokens: 300,
      totalTokens: 24448,
      cachedPromptTokens: 24038,
      details: {
        cachedContentTokenCount: 24038,
        evalDuration: 1200,
        timeToFirstToken: 450,
        promptEvalDuration: 420,
        loadDuration: 100,
      },
    };

    const current: TokenUsage = {
      promptTokens: 8389,
      completionTokens: 24,
      totalTokens: 8413,
      details: {},
    };

    const merged = mergeDisplayTokenUsage(previous, current);

    expect(merged).not.toBeNull();
    expect(merged?.promptTokens).toBe(8389);
    expect(merged?.cachedPromptTokens).toBeUndefined();
    expect(merged?.details?.cachedContentTokenCount).toBeUndefined();
    expect(merged?.details?.evalDuration).toBeUndefined();
    expect(merged?.details?.timeToFirstToken).toBeUndefined();
    expect(merged?.details?.promptEvalDuration).toBeUndefined();
    expect(merged?.details?.loadDuration).toBeUndefined();
  });

  it('falls back to previous metrics when current metrics are null', () => {
    const previous: TokenUsage = {
      promptTokens: 120,
      completionTokens: 45,
      totalTokens: 165,
      details: {
        evalDuration: 321,
      },
    };

    expect(mergeDisplayTokenUsage(previous, null)).toEqual(previous);
  });
});

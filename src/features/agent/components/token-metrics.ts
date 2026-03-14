import type { TokenUsage } from '@/lib/ai-service/types';

export function calculateCacheHitPercent(
  cachedTokens: number,
  promptTokens: number,
): number {
  if (cachedTokens <= 0 || promptTokens <= 0) {
    return 0;
  }

  return Math.min(Math.round((cachedTokens / promptTokens) * 100), 100);
}

export function mergeDisplayTokenUsage(
  lastMetrics: TokenUsage | null,
  metrics: TokenUsage | null,
): TokenUsage | null {
  if (!metrics) {
    return lastMetrics;
  }

  if (!lastMetrics) {
    return metrics;
  }

  const previousDetails = lastMetrics.details ?? {};
  const currentDetails = metrics.details ?? {};

  return {
    ...metrics,
    details: {
      ...currentDetails,
      evalDuration: currentDetails.evalDuration ?? previousDetails.evalDuration,
      timeToFirstToken:
        currentDetails.timeToFirstToken ?? previousDetails.timeToFirstToken,
      promptEvalDuration:
        currentDetails.promptEvalDuration ?? previousDetails.promptEvalDuration,
      loadDuration: currentDetails.loadDuration ?? previousDetails.loadDuration,
    },
  };
}

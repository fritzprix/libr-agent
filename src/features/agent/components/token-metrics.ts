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
  return metrics ?? lastMetrics;
}

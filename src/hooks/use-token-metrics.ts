import { useMemo, useRef } from 'react';
import { useStreamingMessage } from '@/context/LLMServiceContext';
import type { TokenUsage } from '@/lib/ai-service/types';

function areTokenMetricsEqual(
  a: TokenUsage | null,
  b: TokenUsage | null,
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.promptTokens === b.promptTokens &&
    a.completionTokens === b.completionTokens &&
    a.totalTokens === b.totalTokens &&
    a.cachedPromptTokens === b.cachedPromptTokens
  );
}

export function useTokenMetrics(sessionId: string | undefined): {
  metrics: TokenUsage | null;
} {
  const streamingMessage = useStreamingMessage(sessionId);
  const rawUsage = streamingMessage?.usage ?? null;
  const lastMetricsRef = useRef<TokenUsage | null>(null);

  const metrics = useMemo((): TokenUsage | null => {
    if (areTokenMetricsEqual(lastMetricsRef.current, rawUsage)) {
      return lastMetricsRef.current;
    }
    lastMetricsRef.current = rawUsage;
    return rawUsage;
  }, [rawUsage]);

  return useMemo(() => ({ metrics }), [metrics]);
}

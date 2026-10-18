import { useMemo, useRef } from 'react';
import { useStreamingMessage } from '@/context/LLMServiceContext';
import type { TokenUsage } from '@/lib/ai-service/types';

function areDetailsEqual(
  a: TokenUsage['details'] | undefined,
  b: TokenUsage['details'] | undefined,
): boolean {
  if (a === b) return true;
  if (!a || !b) {
    const aKeys = a
      ? Object.keys(a).filter(
          (k) => (a as Record<string, unknown>)[k] !== undefined,
        )
      : [];
    const bKeys = b
      ? Object.keys(b).filter(
          (k) => (b as Record<string, unknown>)[k] !== undefined,
        )
      : [];
    return aKeys.length === 0 && bKeys.length === 0;
  }
  const aRec = a as Record<string, unknown>;
  const bRec = b as Record<string, unknown>;
  const aKeys = Object.keys(aRec).filter((k) => aRec[k] !== undefined);
  const bKeys = Object.keys(bRec).filter((k) => bRec[k] !== undefined);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => aRec[key] === bRec[key]);
}

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
    a.cachedPromptTokens === b.cachedPromptTokens &&
    areDetailsEqual(a.details, b.details)
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

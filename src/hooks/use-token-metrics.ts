import { useMemo } from 'react';
import { useLLMService } from '@/context/llm-service';
import type { TokenUsage } from '@/lib/ai-service/types';

export function useTokenMetrics(sessionId: string | undefined): {
  metrics: TokenUsage | null;
} {
  const { streamingMessages } = useLLMService();

  const metrics = useMemo((): TokenUsage | null => {
    if (!sessionId) return null;
    const message = streamingMessages.get(sessionId);
    return message?.usage ?? null;
  }, [sessionId, streamingMessages]);

  return { metrics };
}

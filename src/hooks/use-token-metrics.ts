import { useMemo } from 'react';
import { useStreamingMessage } from '@/context/LLMServiceContext';
import type { TokenUsage } from '@/lib/ai-service/types';

export function useTokenMetrics(sessionId: string | undefined): {
  metrics: TokenUsage | null;
} {
  const streamingMessage = useStreamingMessage(sessionId);

  const metrics = useMemo((): TokenUsage | null => {
    return streamingMessage?.usage ?? null;
  }, [streamingMessage]);

  return { metrics };
}

import { useAgentChatMessages } from '@/context/AgentChatContext';
import { useTokenMetrics } from '@/hooks/use-token-metrics';
import type { TokenUsage } from '@/lib/ai-service/types';
import type { PreflightTokenMetrics } from '@/models/agent-ipc';
import type { Message } from '@/models/chat';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { TokenMetricsBadge } from '../TokenMetricsBadge';
import { mergeDisplayTokenUsage } from '../token-metrics';

function hasTokenUsageData(
  usage: TokenUsage | null | undefined,
): usage is TokenUsage {
  if (!usage) {
    return false;
  }

  return (
    usage.promptTokens > 0 ||
    usage.completionTokens > 0 ||
    usage.totalTokens > 0 ||
    (usage.cachedPromptTokens ?? 0) > 0
  );
}

function mergePersistedTokenUsage(
  previousUsage: TokenUsage | null,
  nextUsage: TokenUsage,
): TokenUsage {
  if (!previousUsage) {
    return nextUsage;
  }

  return {
    ...previousUsage,
    ...nextUsage,
    details: {
      ...previousUsage.details,
      ...nextUsage.details,
      evalDuration:
        nextUsage.details?.evalDuration ?? previousUsage.details?.evalDuration,
      timeToFirstToken:
        nextUsage.details?.timeToFirstToken ??
        previousUsage.details?.timeToFirstToken,
      promptEvalDuration:
        nextUsage.details?.promptEvalDuration ??
        previousUsage.details?.promptEvalDuration,
      loadDuration:
        nextUsage.details?.loadDuration ?? previousUsage.details?.loadDuration,
    },
  };
}

function findLatestAssistantUsage(messages: Message[]): TokenUsage | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];

    if (message?.role === 'assistant' && hasTokenUsageData(message.usage)) {
      return message.usage;
    }
  }

  return null;
}

export interface StatusBarTokenMetricsProps {
  sessionId?: string;
  preflightTokenMetrics: PreflightTokenMetrics | null | undefined;
  compacting: boolean;
  awaitingCompact: boolean;
  compact: boolean;
}

export function StatusBarTokenMetrics({
  sessionId,
  preflightTokenMetrics,
  compacting,
  awaitingCompact,
  compact,
}: StatusBarTokenMetricsProps) {
  const { metrics } = useTokenMetrics(sessionId);

  // Persist last metrics to show after streaming ends.
  const [persistedMetrics, setPersistedMetrics] = useState<{
    sessionId?: string;
    usage: TokenUsage | null;
  }>({
    sessionId,
    usage: null,
  });

  const [persistedStablePreflight, setPersistedStablePreflight] = useState<{
    sessionId?: string;
    metrics: typeof preflightTokenMetrics;
  }>({
    sessionId,
    metrics: null,
  });

  const lastObservedMetricsRef = useRef<{
    sessionId?: string;
    usage: TokenUsage | null;
  }>({
    sessionId,
    usage: null,
  });

  const persistMetrics = useCallback(
    (usage: TokenUsage) => {
      if (!sessionId) {
        return;
      }

      setPersistedMetrics((previous) => ({
        sessionId,
        usage: mergePersistedTokenUsage(
          previous.sessionId === sessionId ? previous.usage : null,
          usage,
        ),
      }));
    },
    [sessionId],
  );

  useEffect(() => {
    if (!sessionId) {
      lastObservedMetricsRef.current = {
        sessionId: undefined,
        usage: null,
      };
      return;
    }

    const previousObserved = lastObservedMetricsRef.current;

    if (previousObserved.sessionId !== sessionId) {
      lastObservedMetricsRef.current = {
        sessionId,
        usage: metrics,
      };

      if (hasTokenUsageData(metrics)) {
        persistMetrics(metrics);
      }

      return;
    }

    if (hasTokenUsageData(metrics)) {
      persistMetrics(metrics);
    } else if (!metrics && hasTokenUsageData(previousObserved.usage)) {
      persistMetrics(previousObserved.usage);
    }

    lastObservedMetricsRef.current = {
      sessionId,
      usage: metrics,
    };
  }, [metrics, persistMetrics, sessionId]);

  useEffect(() => {
    if (!sessionId) {
      setPersistedStablePreflight({
        sessionId: undefined,
        metrics: null,
      });
      return;
    }

    if (awaitingCompact || compacting || !preflightTokenMetrics) {
      return;
    }

    setPersistedStablePreflight({
      sessionId,
      metrics: preflightTokenMetrics,
    });
  }, [compacting, awaitingCompact, preflightTokenMetrics, sessionId]);

  const sessionPersistedUsage =
    persistedMetrics.sessionId === sessionId ? persistedMetrics.usage : null;
  const sessionPersistedStablePreflight =
    persistedStablePreflight.sessionId === sessionId
      ? persistedStablePreflight.metrics
      : null;
  const badgePreflightMetrics =
    awaitingCompact || compacting
      ? (sessionPersistedStablePreflight ?? preflightTokenMetrics)
      : preflightTokenMetrics;

  const { messages } = useAgentChatMessages();
  const latestAssistantUsage = useMemo(
    () => findLatestAssistantUsage(messages),
    [messages],
  );

  const displayMetrics = useMemo(
    () =>
      mergeDisplayTokenUsage(
        mergeDisplayTokenUsage(sessionPersistedUsage, latestAssistantUsage),
        metrics ?? null,
      ),
    [latestAssistantUsage, metrics, sessionPersistedUsage],
  );

  if (!displayMetrics) {
    return null;
  }

  return (
    <TokenMetricsBadge
      usage={displayMetrics}
      preflight={badgePreflightMetrics}
      className="shrink-0"
      compact={compact}
    />
  );
}

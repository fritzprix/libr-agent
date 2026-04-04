import { createId } from '@paralleldrive/cuid2';
import { AIServiceProvider, TokenUsage } from '../types';
import {
  type Message,
  type RustMessage,
  rustMessageToMessage,
} from '@/models/chat';
import { formatNumber } from '@/lib/utils';

export function isAIServiceProvider(
  value: unknown,
): value is AIServiceProvider {
  return Object.values(AIServiceProvider).includes(value as AIServiceProvider);
}

export function tryParse<T = unknown>(input?: string): T | undefined {
  if (!input) return undefined;
  try {
    return JSON.parse(input) as T;
  } catch {
    return undefined;
  }
}

export function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value ?? {});
  } catch {
    return '{}';
  }
}

export function formatToolCall(id: string, name: string, args: unknown) {
  return {
    id,
    function: {
      name,
      arguments: safeJsonStringify(args),
    },
  };
}

export function generateToolCallId(): string {
  return `tool_${createId()}`;
}

export function normalizeRustMessage(msg: RustMessage | Message): Message {
  const candidate = msg as RustMessage;

  if (
    'toolCalls' in candidate ||
    'toolCallId' in candidate ||
    typeof candidate.createdAt === 'number'
  ) {
    return rustMessageToMessage(candidate);
  }

  return msg as Message;
}

export function calculateTokensPerSecond(
  usage: TokenUsage,
  durationMs: number,
): number {
  if (usage.completionTokens === 0 || durationMs === 0) return 0;
  return (usage.completionTokens / durationMs) * 1000;
}

export function formatUsageMetrics(usage: TokenUsage): {
  input: string;
  output: string;
  total: string;
  cacheHit?: string;
  speed?: string;
} {
  const cached =
    usage.cachedPromptTokens ?? usage.details?.cacheReadInputTokens;
  const cacheHitPercent =
    cached !== undefined && usage.promptTokens > 0
      ? Math.min((cached / usage.promptTokens) * 100, 100).toFixed(0)
      : undefined;

  return {
    input: formatNumber(usage.promptTokens),
    output: formatNumber(usage.completionTokens),
    total: formatNumber(usage.totalTokens),
    cacheHit: cacheHitPercent ? `${cacheHitPercent}%` : undefined,
    speed: usage.details?.evalDuration
      ? `${((usage.completionTokens / usage.details.evalDuration) * 1000).toFixed(1)} t/s`
      : undefined,
  };
}

export function isSpendingCapError(error: unknown): boolean {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'object' &&
          error !== null &&
          'message' in error &&
          typeof error.message === 'string'
        ? error.message
        : String(error);
  return (
    message.includes('spending cap') ||
    (message.includes('RESOURCE_EXHAUSTED') && message.includes('spending'))
  );
}

import { createId } from '@paralleldrive/cuid2';
import {
  AIServiceError,
  AIServiceProvider,
  type AIServiceErrorKind,
  TokenUsage,
} from '../types';
import {
  type Message,
  type MessageError,
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function getString(
  record: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = record[key];
  return typeof value === 'string' ? value : undefined;
}

function getNumber(
  record: Record<string, unknown>,
  key: string,
): number | undefined {
  const value = record[key];
  return typeof value === 'number' ? value : undefined;
}

function tryParseJsonObject(
  input: string,
): Record<string, unknown> | undefined {
  try {
    const parsed: unknown = JSON.parse(input);
    return isRecord(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

function parseProviderErrorPayload(input: string): {
  message?: string;
  code?: number;
  status?: string;
} | null {
  const trimmed = input.trim();
  const jsonStart = trimmed.indexOf('{');
  const candidates =
    jsonStart >= 0 ? [trimmed, trimmed.slice(jsonStart)] : [trimmed];

  for (const candidate of candidates) {
    const parsed = tryParseJsonObject(candidate);
    if (!parsed) continue;

    const errorRecord = isRecord(parsed.error) ? parsed.error : parsed;
    const code = getNumber(errorRecord, 'code');
    const status = getString(errorRecord, 'status');
    const message = getString(errorRecord, 'message');

    if (message) {
      const nested = parseProviderErrorPayload(message);
      return {
        code: nested?.code ?? code,
        status: nested?.status ?? status,
        message: nested?.message ?? message,
      };
    }

    if (code !== undefined || status !== undefined) {
      return { code, status };
    }
  }

  return null;
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (isRecord(error)) {
    const message = getString(error, 'message');
    if (message) {
      return message;
    }
  }

  return String(error);
}

function extractFirstUrl(input: string): string | undefined {
  const match = input.match(/https?:\/\/[^\s)"]+/i);
  return match?.[0];
}

export function normalizeAIServiceError(error: unknown): {
  type: MessageError['type'];
  displayMessage: string;
  recoverable: boolean;
  errorCode?: string;
} | null {
  if (error instanceof AIServiceError) {
    const mapped = mapTypedAIServiceError(error);
    if (mapped) {
      return mapped;
    }
  }

  const rawMessage = getErrorMessage(error);
  const parsedPayload = parseProviderErrorPayload(rawMessage);
  const providerMessage = parsedPayload?.message?.trim();
  const normalizedProviderMessage =
    providerMessage && !providerMessage.startsWith('{')
      ? providerMessage.replace(/\s+/g, ' ').trim()
      : undefined;
  const billingUrl = extractFirstUrl(normalizedProviderMessage ?? rawMessage);
  const status = parsedPayload?.status;
  const code = parsedPayload?.code;
  const lowerRawMessage = rawMessage.toLowerCase();
  const isRateLimit =
    code === 429 ||
    status === 'RESOURCE_EXHAUSTED' ||
    lowerRawMessage.includes('rate limit') ||
    lowerRawMessage.includes('too many requests');

  if (isSpendingCapError(error)) {
    const billingMessage = billingUrl
      ? `Billing limit reached for this AI provider. Update your billing or quota settings and try again: ${billingUrl}`
      : 'Billing limit reached for this AI provider. Update your billing or quota settings and try again.';

    return {
      type: 'RATE_LIMIT_ERROR',
      displayMessage: billingMessage,
      recoverable: false,
      errorCode: 'SPENDING_CAP_EXCEEDED',
    };
  }

  if (isRateLimit) {
    return {
      type: 'RATE_LIMIT_ERROR',
      displayMessage:
        normalizedProviderMessage ??
        'Rate limit exceeded. Please wait a moment and try again.',
      recoverable: true,
      errorCode: 'RATE_LIMIT_EXCEEDED',
    };
  }

  return null;
}

function mapTypedAIServiceError(error: AIServiceError): {
  type: MessageError['type'];
  displayMessage: string;
  recoverable: boolean;
  errorCode?: string;
} | null {
  const kind = error.metadata.kind;
  if (!kind) {
    return null;
  }

  const providerMessage = error.message
    .replace(/^[^:]+ streaming failed:\s*/i, '')
    .trim();

  const byKind: Record<
    AIServiceErrorKind,
    {
      type: MessageError['type'];
      recoverable: boolean;
      errorCode: string;
    }
  > = {
    context_limit: {
      type: 'CONTEXT_LIMIT_ERROR',
      recoverable: true,
      errorCode: 'CONTEXT_LIMIT_EXCEEDED',
    },
    rate_limit: {
      type: 'RATE_LIMIT_ERROR',
      recoverable: error.metadata.retryable ?? true,
      errorCode: isSpendingCapError(error)
        ? 'SPENDING_CAP_EXCEEDED'
        : 'RATE_LIMIT_EXCEEDED',
    },
    authentication: {
      type: 'AUTHENTICATION_ERROR',
      recoverable: false,
      errorCode: 'AUTHENTICATION_FAILED',
    },
    network: {
      type: 'NETWORK_ERROR',
      recoverable: true,
      errorCode: 'NETWORK_ERROR',
    },
    invalid_request: {
      type: 'VALIDATION_ERROR',
      recoverable: false,
      errorCode: 'INVALID_REQUEST',
    },
    server: {
      type: 'AI_SERVICE_ERROR',
      recoverable: true,
      errorCode: 'PROVIDER_SERVER_ERROR',
    },
    unknown: {
      type: 'AI_SERVICE_ERROR',
      recoverable: true,
      errorCode: 'AI_SERVICE_ERROR',
    },
  };

  return {
    type: byKind[kind].type,
    displayMessage: providerMessage || error.message,
    recoverable: byKind[kind].recoverable,
    errorCode: byKind[kind].errorCode,
  };
}

import { isSpendingCapError } from '@/lib/ai-service/utils';
import {
  AIServiceError,
  type AIServiceConfig,
  type AIServiceErrorKind,
  type AIServiceErrorMetadata,
  type AIServiceProvider,
} from './types';
import type { StreamingErrorContext } from './base-service-shared';

interface RetryLogger {
  warn: (message: string, ...args: unknown[]) => unknown;
}

interface StreamingErrorLogger {
  info: (message: string, ...args: unknown[]) => unknown;
  error: (message: string, ...args: unknown[]) => unknown;
}

export function createAbortError(): Error {
  const abortError = new Error('Request aborted');
  abortError.name = 'AbortError';
  return abortError;
}

export function shouldRetryRequest(error: unknown): boolean {
  if (error instanceof AIServiceError) {
    if (error.metadata.retryable !== undefined) {
      return error.metadata.retryable;
    }
    if (error.statusCode === 429) {
      return error.metadata.kind !== 'rate_limit' || !isSpendingCapError(error);
    }
    if (error.metadata.kind === 'context_limit') {
      return false;
    }
  }

  const status = (error as { status?: number })?.status;
  if (status === undefined) {
    return false;
  }

  if (status === 429) {
    return !isSpendingCapError(error);
  }

  return status >= 500 && status <= 599;
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

function getNestedRecord(
  record: Record<string, unknown>,
  key: string,
): Record<string, unknown> | undefined {
  const value = record[key];
  return isRecord(value) ? value : undefined;
}

function parseStructuredProviderError(error: unknown): {
  statusCode?: number;
  providerCode?: string | number;
  providerStatus?: string;
  providerMessage?: string;
  rawPayload?: unknown;
} {
  if (!isRecord(error)) {
    return {};
  }

  const topLevelError = getNestedRecord(error, 'error');
  const errorRecord = topLevelError ?? error;
  const providerMessage =
    getString(errorRecord, 'message') ?? getString(error, 'message');
  const providerStatus =
    getString(errorRecord, 'type') ?? getString(errorRecord, 'status');
  const providerCode =
    getString(errorRecord, 'code') ??
    getNumber(errorRecord, 'code') ??
    getString(error, 'code') ??
    getNumber(error, 'code');
  const statusCode =
    getNumber(error, 'status') ?? getNumber(errorRecord, 'status');

  return {
    statusCode,
    providerCode,
    providerStatus,
    providerMessage,
    rawPayload: topLevelError ?? error,
  };
}

function classifyProviderErrorKind(args: {
  providerMessage?: string;
  providerStatus?: string;
  providerCode?: string | number;
  statusCode?: number;
  error: unknown;
}): AIServiceErrorKind {
  if (isSpendingCapError(args.error)) {
    return 'rate_limit';
  }

  if (args.statusCode === 429) {
    return 'rate_limit';
  }

  if (args.statusCode === 401 || args.statusCode === 403) {
    return 'authentication';
  }

  const normalizedStatus = args.providerStatus?.toLowerCase();
  const normalizedCode =
    typeof args.providerCode === 'string'
      ? args.providerCode.toLowerCase()
      : undefined;

  if (
    normalizedStatus === 'context_length_exceeded' ||
    normalizedStatus === 'context_window_exceeded' ||
    normalizedCode === 'context_length_exceeded' ||
    normalizedCode === 'context_window_exceeded'
  ) {
    return 'context_limit';
  }

  const normalizedMessage = args.providerMessage?.toLowerCase();
  if (
    normalizedMessage?.includes('context size has been exceeded') ||
    normalizedMessage?.includes('maximum context length') ||
    normalizedMessage?.includes('context window exceeded') ||
    normalizedMessage?.includes('prompt is too long')
  ) {
    return 'context_limit';
  }

  if (
    normalizedStatus === 'invalid_request_error' ||
    normalizedCode === 'invalid_request_error' ||
    args.statusCode === 400
  ) {
    return 'invalid_request';
  }

  if (normalizedMessage?.includes('connection error')) {
    return 'network';
  }

  if (
    args.statusCode !== undefined &&
    args.statusCode >= 500 &&
    args.statusCode <= 599
  ) {
    return 'server';
  }

  return 'unknown';
}

function buildAIServiceErrorMetadata(error: unknown): {
  statusCode?: number;
  metadata: AIServiceErrorMetadata;
} {
  const parsed = parseStructuredProviderError(error);
  const kind = classifyProviderErrorKind({
    ...parsed,
    error,
  });
  const retryable =
    kind === 'network' ||
    kind === 'server' ||
    (kind === 'rate_limit' && !isSpendingCapError(error));

  return {
    statusCode: parsed.statusCode,
    metadata: {
      kind,
      retryable,
      providerCode: parsed.providerCode,
      providerStatus: parsed.providerStatus,
      rawPayload: parsed.rawPayload,
    },
  };
}

export async function withRetryPolicy<T>(args: {
  fn: () => Promise<T>;
  config: AIServiceConfig;
  abortSignal?: AbortSignal;
  logger: RetryLogger;
  provider: AIServiceProvider;
  shouldRetry: (error: unknown) => boolean;
}): Promise<T> {
  const maxRetries = args.config.maxRetries ?? 3;
  let lastError: unknown;

  for (let i = 0; i <= maxRetries; i += 1) {
    try {
      return await args.fn();
    } catch (error: unknown) {
      lastError = error;

      if (args.abortSignal?.aborted) {
        throw error;
      }

      if (args.shouldRetry(error) && i < maxRetries) {
        const delay = Math.pow(2, i) * (args.config.retryDelay ?? 1000);
        args.logger.warn(
          `Retrying request (${i + 1}/${maxRetries}) after ${delay}ms...`,
        );
        await new Promise<void>((resolve, reject) => {
          if (args.abortSignal?.aborted) {
            reject(createAbortError());
            return;
          }

          const timeoutId = setTimeout(() => {
            args.abortSignal?.removeEventListener('abort', onAbort);
            resolve();
          }, delay);

          const onAbort = () => {
            clearTimeout(timeoutId);
            args.abortSignal?.removeEventListener('abort', onAbort);
            reject(createAbortError());
          };

          args.abortSignal?.addEventListener('abort', onAbort, { once: true });
        });
        continue;
      }

      throw error;
    }
  }

  if (lastError instanceof AIServiceError) {
    throw lastError;
  }

  if (lastError instanceof Error) {
    const { statusCode, metadata } = buildAIServiceErrorMetadata(lastError);
    throw new AIServiceError(
      lastError.message,
      args.provider,
      statusCode,
      lastError,
      metadata,
    );
  }

  const { statusCode, metadata } = buildAIServiceErrorMetadata(lastError);
  throw new AIServiceError(
    String(lastError),
    args.provider,
    statusCode,
    undefined,
    metadata,
  );
}

export function throwStreamingError(args: {
  error: unknown;
  context: StreamingErrorContext;
  abortSignal?: AbortSignal;
  logger: StreamingErrorLogger;
  provider: AIServiceProvider;
}): never {
  const errorMessage =
    args.error instanceof Error ? args.error.message : 'Unknown error';
  const errorStack = args.error instanceof Error ? args.error.stack : undefined;

  const isCancellation =
    args.abortSignal?.aborted ||
    (args.error instanceof Error &&
      (args.error.name === 'AbortError' ||
        args.error.message.includes('abort') ||
        args.error.message.includes('cancel')));

  if (isCancellation) {
    args.logger.info(`${args.provider} stream cancelled by user`);
    throw createAbortError();
  }

  args.logger.error(`${args.provider} streaming failed`, {
    error: errorMessage,
    stack: errorStack,
    requestData: {
      model: args.context.options.modelName || args.context.config.defaultModel,
      messagesCount: args.context.messages.length,
      hasTools: !!args.context.options.availableTools?.length,
      systemPrompt: !!args.context.options.systemPrompt,
    },
  });

  const { statusCode, metadata } = buildAIServiceErrorMetadata(args.error);
  throw new AIServiceError(
    `${args.provider} streaming failed: ${errorMessage}`,
    args.provider,
    statusCode,
    args.error instanceof Error ? args.error : undefined,
    metadata,
  );
}

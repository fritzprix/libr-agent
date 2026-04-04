import { isSpendingCapError } from '@/lib/ai-service/utils';
import {
  AIServiceError,
  type AIServiceConfig,
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

export function shouldRetryRequest(error: unknown): boolean {
  const status = (error as { status?: number })?.status;
  if (status === undefined) {
    return false;
  }

  if (status === 429) {
    return !isSpendingCapError(error);
  }

  return status >= 500 && status <= 599;
}

export async function withRetryPolicy<T>(args: {
  fn: () => Promise<T>;
  config: AIServiceConfig;
  abortSignal: AbortSignal;
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

      if (args.abortSignal.aborted) {
        throw error;
      }

      if (args.shouldRetry(error) && i < maxRetries) {
        const delay = Math.pow(2, i) * (args.config.retryDelay ?? 1000);
        args.logger.warn(
          `Retrying request (${i + 1}/${maxRetries}) after ${delay}ms...`,
        );
        await new Promise((resolve) => setTimeout(resolve, delay));
        continue;
      }

      throw error;
    }
  }

  if (lastError instanceof Error) {
    throw new AIServiceError(
      lastError.message,
      args.provider,
      undefined,
      lastError,
    );
  }

  throw new AIServiceError(String(lastError), args.provider);
}

export function throwStreamingError(args: {
  error: unknown;
  context: StreamingErrorContext;
  abortSignal: AbortSignal;
  logger: StreamingErrorLogger;
  provider: AIServiceProvider;
}): never {
  const errorMessage =
    args.error instanceof Error ? args.error.message : 'Unknown error';
  const errorStack = args.error instanceof Error ? args.error.stack : undefined;

  const isCancellation =
    args.abortSignal.aborted ||
    (args.error instanceof Error &&
      (args.error.name === 'AbortError' ||
        args.error.message.includes('abort') ||
        args.error.message.includes('cancel')));

  if (isCancellation) {
    args.logger.info(`${args.provider} stream cancelled by user`);
    throw new AIServiceError(
      `${args.provider} stream cancelled`,
      args.provider,
      undefined,
      args.error instanceof Error ? args.error : undefined,
    );
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

  throw new AIServiceError(
    `${args.provider} streaming failed: ${errorMessage}`,
    args.provider,
    undefined,
    args.error instanceof Error ? args.error : undefined,
  );
}

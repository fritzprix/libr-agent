/**
 * Common retry utility functions for handling retries with exponential backoff
 */

export interface RetryOptions {
  maxRetries?: number;
  baseDelay?: number;
  maxDelay?: number;
  timeout?: number;
  exponentialBackoff?: boolean;
}

export interface RetryResult<T> {
  success: boolean;
  result?: T;
  error?: Error;
  attemptCount: number;
}

/**
 * Sleep utility for delays
 */
export const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Execute operation with timeout
 */
export const withTimeout = async <T>(
  promise: Promise<T>,
  timeoutMs: number,
): Promise<T> => {
  const timeoutPromise = new Promise<never>((_, reject) => {
    setTimeout(() => reject(new Error('Operation timed out')), timeoutMs);
  });

  return Promise.race([promise, timeoutPromise]);
};

/**
 * Execute operation with retry logic
 */
export const withRetry = async <T>(
  operation: () => Promise<T>,
  options: RetryOptions = {},
): Promise<T> => {
  const {
    maxRetries = 3,
    baseDelay = 1000,
    maxDelay = 30000,
    timeout,
    exponentialBackoff = true,
  } = options;

  let lastError: Error;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const operationPromise = operation();
      const result = timeout
        ? await withTimeout(operationPromise, timeout)
        : await operationPromise;

      return result;
    } catch (error) {
      lastError = error as Error;

      if (attempt === maxRetries) {
        throw new Error(
          `Operation failed after ${maxRetries + 1} attempts: ${lastError.message}`,
        );
      }

      // Calculate delay
      const delay = exponentialBackoff
        ? Math.min(baseDelay * Math.pow(2, attempt), maxDelay)
        : baseDelay;

      await sleep(delay);
    }
  }

  throw lastError!;
};

/**
 * Execute operation with retry logic and return detailed result
 */
export const withRetryResult = async <T>(
  operation: () => Promise<T>,
  options: RetryOptions = {},
): Promise<RetryResult<T>> => {
  const {
    maxRetries = 3,
    baseDelay = 1000,
    maxDelay = 30000,
    timeout,
    exponentialBackoff = true,
  } = options;

  let lastError: Error;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const operationPromise = operation();
      const result = timeout
        ? await withTimeout(operationPromise, timeout)
        : await operationPromise;

      return {
        success: true,
        result,
        attemptCount: attempt + 1,
      };
    } catch (error) {
      lastError = error as Error;

      if (attempt === maxRetries) {
        return {
          success: false,
          error: lastError,
          attemptCount: attempt + 1,
        };
      }

      // Calculate delay
      const delay = exponentialBackoff
        ? Math.min(baseDelay * Math.pow(2, attempt), maxDelay)
        : baseDelay;

      await sleep(delay);
    }
  }

  return {
    success: false,
    error: lastError!,
    attemptCount: maxRetries + 1,
  };
};

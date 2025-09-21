/**
 * @file Common utility functions for handling retries with exponential backoff.
 */

/**
 * Defines the options for a retryable operation.
 */
export interface RetryOptions {
  /** The maximum number of times to retry the operation. Defaults to 3. */
  maxRetries?: number;
  /** The base delay in milliseconds for the first retry. Defaults to 1000. */
  baseDelay?: number;
  /** The maximum delay in milliseconds between retries. Defaults to 30000. */
  maxDelay?: number;
  /** An optional timeout for each attempt in milliseconds. */
  timeout?: number;
  /** If true, uses exponential backoff for delays. Defaults to true. */
  exponentialBackoff?: boolean;
}

/**
 * Represents the detailed result of a retryable operation.
 * @template T The type of the result if the operation is successful.
 */
export interface RetryResult<T> {
  /** Indicates whether the operation was successful. */
  success: boolean;
  /** The result of the operation if successful. */
  result?: T;
  /** The error that occurred if the operation failed. */
  error?: Error;
  /** The total number of attempts made. */
  attemptCount: number;
}

/**
 * A simple sleep utility that pauses execution for a specified duration.
 * @param ms The number of milliseconds to sleep.
 * @returns A promise that resolves after the specified duration.
 */
export const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Executes an asynchronous operation with a specified timeout.
 * If the operation does not complete within the timeout period, it will be rejected.
 * @template T The type of the result of the promise.
 * @param promise The promise representing the asynchronous operation.
 * @param timeoutMs The timeout in milliseconds.
 * @returns A promise that resolves with the result of the original promise,
 *          or rejects if the timeout is exceeded.
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
 * Executes an asynchronous operation with a retry mechanism.
 * If the operation fails, it will be retried according to the specified options.
 * This function throws an error if the operation fails after all retries.
 *
 * @template T The type of the result of the operation.
 * @param operation A function that returns a promise for the operation to be executed.
 * @param options The options for the retry logic.
 * @returns A promise that resolves with the result of the successful operation.
 * @throws An error if the operation fails after all retry attempts.
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
 * Executes an asynchronous operation with a retry mechanism and returns a detailed result object.
 * This function does not throw an error on failure; instead, it returns an object
 * indicating the success or failure of the operation.
 *
 * @template T The type of the result of the operation.
 * @param operation A function that returns a promise for the operation to be executed.
 * @param options The options for the retry logic.
 * @returns A promise that resolves to a `RetryResult` object, which contains
 *          the outcome of the operation and the number of attempts.
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

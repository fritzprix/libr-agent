import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { sleep, withTimeout, withRetry, withRetryResult } from '../retry-utils';

describe('retry-utils', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('sleep', () => {
    it('should wait for specified duration', async () => {
      const promise = sleep(1000);
      // Timer hasn't advanced yet
      let resolved = false;
      promise.then(() => { resolved = true; });
      expect(resolved).toBe(false);

      await vi.advanceTimersByTimeAsync(1000);
      expect(resolved).toBe(true);
    });
  });

  describe('withTimeout', () => {
    it('should resolve if operation completes in time', async () => {
      const operation = Promise.resolve('success');
      await expect(withTimeout(operation, 1000)).resolves.toBe('success');
    });

    it('should reject if operation times out', async () => {
      // Create a promise that never resolves (or takes too long)
      const operation = new Promise(resolve => setTimeout(resolve, 2000));
      const promise = withTimeout(operation, 1000);

      const assertPromise = expect(promise).rejects.toThrow('Operation timed out');
      await vi.advanceTimersByTimeAsync(1000);
      await assertPromise;
    });
  });

  describe('withRetry', () => {
    it('should succeed on first try', async () => {
      const operation = vi.fn().mockResolvedValue('success');
      const result = await withRetry(operation);
      expect(result).toBe('success');
      expect(operation).toHaveBeenCalledTimes(1);
    });

    it('should retry on failure and succeed', async () => {
      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail 1'))
        .mockResolvedValue('success');

      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 3 });

      // Advance enough time for the first retry delay (100ms)
      await vi.advanceTimersByTimeAsync(100);

      await expect(promise).resolves.toBe('success');
      expect(operation).toHaveBeenCalledTimes(2);
    });

    it('should fail after max retries', async () => {
      const operation = vi.fn().mockRejectedValue(new Error('fail'));
      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2 });

      const assertPromise = expect(promise).rejects.toThrow('Operation failed after 3 attempts: fail');

      await vi.advanceTimersByTimeAsync(1000);

      await assertPromise;
      expect(operation).toHaveBeenCalledTimes(3);
    });

    it('should use exponential backoff', async () => {
       const operation = vi.fn().mockRejectedValue(new Error('fail'));
       const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2, exponentialBackoff: true });

       const assertPromise = expect(promise).rejects.toThrow('Operation failed after 3 attempts: fail');

       // Attempt 1 fails immediately.
       // Delay 1: 100ms.

       // Check that operation was called once
       expect(operation).toHaveBeenCalledTimes(1);

       // Advance time by 50ms. Still waiting.
       await vi.advanceTimersByTimeAsync(50);
       expect(operation).toHaveBeenCalledTimes(1);

       // Advance time by another 60ms (total 110ms). Should have retried.
       await vi.advanceTimersByTimeAsync(60);
       expect(operation).toHaveBeenCalledTimes(2);

       // Attempt 2 fails.
       // Delay 2: 200ms.

       // Advance time by 100ms. Still waiting.
       await vi.advanceTimersByTimeAsync(100);
       expect(operation).toHaveBeenCalledTimes(2);

       // Advance time by another 110ms (total 210ms). Should have retried.
       await vi.advanceTimersByTimeAsync(110);
       expect(operation).toHaveBeenCalledTimes(3);

       // Attempt 3 fails. Max retries reached. Should throw.
       await assertPromise;
    });
  });

  describe('withRetryResult', () => {
    it('should return success result', async () => {
       const operation = vi.fn().mockResolvedValue('success');
       const result = await withRetryResult(operation);
       expect(result).toEqual({
         success: true,
         result: 'success',
         attemptCount: 1
       });
    });

    it('should return failure result after retries', async () => {
       const operation = vi.fn().mockRejectedValue(new Error('fail'));
       const promise = withRetryResult(operation, { baseDelay: 100, maxRetries: 2 });

       await vi.advanceTimersByTimeAsync(1000);

       const result = await promise;
       expect(result).toEqual({
         success: false,
         error: expect.any(Error),
         attemptCount: 3
       });
       expect(result.error?.message).toBe('fail');
    });
  });
});

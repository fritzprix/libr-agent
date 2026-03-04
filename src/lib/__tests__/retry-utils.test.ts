import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { sleep, withTimeout, withRetry, withRetryResult } from '../retry-utils';

describe('retry-utils', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
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

  // SP4 regression tests
  describe('jitter option', () => {
    it('withRetry: jitter=true scales delay by Math.random factor (min half)', async () => {
      // Math.random() = 0 → multiplier = 0.5 → delay = baseDelay * 0.5 = 50ms
      vi.spyOn(Math, 'random').mockReturnValue(0);

      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail'))
        .mockResolvedValue('ok');

      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2, jitter: true });

      // Before jittered delay (50ms) elapses, operation has only been called once
      await vi.advanceTimersByTimeAsync(30);
      expect(operation).toHaveBeenCalledTimes(1);

      // After 50ms jittered delay, retry fires
      await vi.advanceTimersByTimeAsync(30); // total 60ms > 50ms jitter
      await expect(promise).resolves.toBe('ok');
      expect(operation).toHaveBeenCalledTimes(2);
      vi.restoreAllMocks();
    });

    it('withRetry: jitter=true scales delay by Math.random factor (max 1.5x)', async () => {
      // Math.random() = 1 → multiplier = 1.5 → delay = baseDelay * 1.5 = 150ms
      vi.spyOn(Math, 'random').mockReturnValue(1);

      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail'))
        .mockResolvedValue('ok');

      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2, jitter: true });

      // Below exact (non-jittered) delay of 100ms — operation should NOT yet have retried
      await vi.advanceTimersByTimeAsync(110);
      expect(operation).toHaveBeenCalledTimes(1);

      // After full jittered delay of 150ms, retry fires
      await vi.advanceTimersByTimeAsync(50); // total 160ms > 150ms
      await expect(promise).resolves.toBe('ok');
      expect(operation).toHaveBeenCalledTimes(2);
    });

    it('withRetry: jitter=false uses exact exponential delay unchanged', async () => {
      // Without jitter, delay at attempt 0 = baseDelay = 100ms exactly
      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail'))
        .mockResolvedValue('ok');

      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2, jitter: false });

      await vi.advanceTimersByTimeAsync(90);
      expect(operation).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(20); // total 110ms > 100ms
      await expect(promise).resolves.toBe('ok');
      expect(operation).toHaveBeenCalledTimes(2);
    });

    it('withRetryResult: jitter=true returns success with correct attemptCount', async () => {
      vi.spyOn(Math, 'random').mockReturnValue(0); // min delay (0.5x)

      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('transient'))
        .mockResolvedValue('recovered');

      const promise = withRetryResult(operation, { baseDelay: 100, maxRetries: 2, jitter: true });

      await vi.advanceTimersByTimeAsync(60); // past 50ms jitter delay
      const result = await promise;

      expect(result.success).toBe(true);
      expect(result.result).toBe('recovered');
      expect(result.attemptCount).toBe(2);
      vi.restoreAllMocks();
    });
  });

  describe('withRetryResult extra edge cases', () => {
    it('should use baseDelay without exponential backoff', async () => {
       const operation = vi.fn().mockRejectedValue(new Error('fail'));
       const promise = withRetryResult(operation, { baseDelay: 100, maxRetries: 2, exponentialBackoff: false });

       // Attempt 1 fails
       await vi.advanceTimersByTimeAsync(100);
       expect(operation).toHaveBeenCalledTimes(2);

       // Attempt 2 fails
       await vi.advanceTimersByTimeAsync(100);
       expect(operation).toHaveBeenCalledTimes(3);

       const result = await promise;
       expect(result.success).toBe(false);
    });

    it('should use timeout per attempt', async () => {
       const operation = vi.fn().mockImplementation(() => new Promise(resolve => setTimeout(resolve, 2000)));
       const promise = withRetryResult(operation, { timeout: 1000, maxRetries: 0 });

       await vi.advanceTimersByTimeAsync(1000);

       const result = await promise;
       expect(result.success).toBe(false);
       expect(result.error?.message).toBe('Operation timed out');
    });

    it('should use timeout per attempt with withRetry', async () => {
       const operation = vi.fn().mockImplementation(() => new Promise(resolve => setTimeout(resolve, 2000)));
       const promise = withRetry(operation, { timeout: 1000, maxRetries: 0 });

       const assertPromise = expect(promise).rejects.toThrow('Operation failed after 1 attempts: Operation timed out');
       await vi.advanceTimersByTimeAsync(1000);
       await assertPromise;
    });

    it('withRetryResult: jitter=true scales delay by Math.random factor (max 1.5x)', async () => {
      vi.spyOn(Math, 'random').mockReturnValue(1); // max delay (1.5x)

      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('transient'))
        .mockResolvedValue('recovered');

      const promise = withRetryResult(operation, { baseDelay: 100, maxRetries: 2, jitter: true });

      await vi.advanceTimersByTimeAsync(110);
      expect(operation).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(50); // total 160ms
      const result = await promise;

      expect(result.success).toBe(true);
      expect(result.result).toBe('recovered');
      expect(result.attemptCount).toBe(2);
    });

    it('withRetryResult: jitter=false uses exact exponential delay unchanged', async () => {
      // Without jitter, delay at attempt 0 = baseDelay = 100ms exactly
      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail'))
        .mockResolvedValue('ok');

      const promise = withRetryResult(operation, { baseDelay: 100, maxRetries: 2, jitter: false });

      await vi.advanceTimersByTimeAsync(90);
      expect(operation).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(20); // total 110ms > 100ms
      await expect(promise).resolves.toEqual({ success: true, result: 'ok', attemptCount: 2 });
      expect(operation).toHaveBeenCalledTimes(2);
    });



    it('withRetry: uses exact base delay unchanged if exponentialBackoff=false', async () => {
      const operation = vi.fn()
        .mockRejectedValueOnce(new Error('fail'))
        .mockResolvedValue('ok');

      const promise = withRetry(operation, { baseDelay: 100, maxRetries: 2, exponentialBackoff: false, jitter: false });

      await vi.advanceTimersByTimeAsync(90);
      expect(operation).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(20); // total 110ms > 100ms
      await expect(promise).resolves.toBe('ok');
      expect(operation).toHaveBeenCalledTimes(2);
    });

    it('withRetry: resolves if operation completes within timeout', async () => {
      const operation = vi.fn().mockImplementation(() => new Promise(resolve => setTimeout(() => resolve('success'), 500)));
      const promise = withRetry(operation, { timeout: 1000 });

      await vi.advanceTimersByTimeAsync(500);
      await expect(promise).resolves.toBe('success');
    });

    it('withRetryResult: resolves if operation completes within timeout', async () => {
      const operation = vi.fn().mockImplementation(() => new Promise(resolve => setTimeout(() => resolve('success'), 500)));
      const promise = withRetryResult(operation, { timeout: 1000 });

      await vi.advanceTimersByTimeAsync(500);
      await expect(promise).resolves.toEqual({
         success: true,
         result: 'success',
         attemptCount: 1
      });
    });
  });
});

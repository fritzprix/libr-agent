import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  stringToMCPContentArray,
  cn,
  toValidJsName,
  throttlePromise,
} from '../utils';

describe('utils', () => {
  describe('stringToMCPContentArray', () => {
    it('should wrap string in [{ type: "text", text: ... }]', () => {
      const result = stringToMCPContentArray('hello world');
      expect(result).toEqual([{ type: 'text', text: 'hello world' }]);
    });
  });

  describe('cn (className merger)', () => {
    it('should merge class names correctly', () => {
      expect(cn('class1', 'class2')).toBe('class1 class2');
    });

    it('should handle conditional classes', () => {
      expect(cn('class1', false && 'class2', 'class3')).toBe('class1 class3');
      expect(cn('class1', true && 'class2')).toBe('class1 class2');
    });

    it('should merge Tailwind classes (override previous classes)', () => {
      expect(cn('p-4', 'p-2')).toBe('p-2');
      expect(cn('text-red-500', 'text-blue-500')).toBe('text-blue-500');
    });
  });

  describe('toValidJsName', () => {
    it('should replace invalid characters with underscores', () => {
      expect(toValidJsName('invalid-name')).toBe('invalid_name');
      expect(toValidJsName('name with spaces')).toBe('name_with_spaces');
      expect(toValidJsName('hello@world')).toBe('hello_world');
    });

    it('should prefix with underscore if starting with a digit', () => {
      expect(toValidJsName('123start')).toBe('_123start');
    });

    it('should append underscore if it matches a reserved keyword', () => {
      expect(toValidJsName('class')).toBe('class_');
      expect(toValidJsName('function')).toBe('function_');
      expect(toValidJsName('import')).toBe('import_');
    });

    it('should not modify valid names', () => {
      expect(toValidJsName('validName')).toBe('validName');
      expect(toValidJsName('_private')).toBe('_private');
      expect(toValidJsName('$dollar')).toBe('$dollar');
    });
  });

  describe('throttlePromise', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('should execute immediately on first call', async () => {
      const fn = vi.fn().mockResolvedValue('result');
      const throttled = throttlePromise(fn, 1000);

      const promise = throttled();
      expect(fn).toHaveBeenCalledTimes(1);
      await expect(promise).resolves.toBe('result');
    });

    it('should throttle subsequent calls within wait period', async () => {
      const fn = vi.fn(async (arg) => arg);
      const throttled = throttlePromise(fn, 1000);

      throttled(1);
      throttled(2);
      throttled(3);

      expect(fn).toHaveBeenCalledTimes(1); // Only the first call executed immediately
      expect(fn).toHaveBeenCalledWith(1);

      // Fast-forward time by 500ms (less than wait)
      await vi.advanceTimersByTimeAsync(500);
      expect(fn).toHaveBeenCalledTimes(1);

      // Fast-forward another 500ms (total 1000ms)
      await vi.advanceTimersByTimeAsync(500);
      expect(fn).toHaveBeenCalledTimes(2); // The last call (3) should be executed now
      expect(fn).toHaveBeenLastCalledWith(3);
    });

    it('should execute the last call after the wait period', async () => {
      const fn = vi.fn(async (arg) => arg);
      const throttled = throttlePromise(fn, 100);

      throttled('first');
      throttled('second');
      throttled('third');

      expect(fn).toHaveBeenCalledTimes(1);
      expect(fn).toHaveBeenCalledWith('first');

      await vi.advanceTimersByTimeAsync(100);

      expect(fn).toHaveBeenCalledTimes(2);
      expect(fn).toHaveBeenLastCalledWith('third'); // 'second' is skipped
    });
  });
});

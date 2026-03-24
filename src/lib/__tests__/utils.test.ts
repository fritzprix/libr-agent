import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  stringToMCPContentArray,
  cn,
  toValidJsName,
  throttlePromise,
  formatNumber,
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
      const shouldInclude = false;
      const shouldInclude2 = true;
      expect(cn('class1', shouldInclude && 'class2', 'class3')).toBe('class1 class3');
      expect(cn('class1', shouldInclude2 && 'class2')).toBe('class1 class2');
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

      const p1 = throttled(1);
      const p2 = throttled(2);
      const p3 = throttled(3);

      // First call executes immediately
      await expect(p1).resolves.toBe(1);
      expect(fn).toHaveBeenCalledTimes(1);
      expect(fn).toHaveBeenCalledWith(1);

      // Fast-forward time by 500ms (less than wait)
      await vi.advanceTimersByTimeAsync(500);
      expect(fn).toHaveBeenCalledTimes(1);

      // Fast-forward another 500ms (total 1000ms)
      await vi.advanceTimersByTimeAsync(500);
      expect(fn).toHaveBeenCalledTimes(2);
      expect(fn).toHaveBeenLastCalledWith(3);

      // Verify pending promises resolve with the final result
      await expect(p2).resolves.toBe(3);
      await expect(p3).resolves.toBe(3);
    });

    it('should execute the last call after the wait period', async () => {
      const fn = vi.fn(async (arg) => arg);
      const throttled = throttlePromise(fn, 100);

      const p1 = throttled('first');
      const p2 = throttled('second');
      const p3 = throttled('third');

      // First call executes immediately
      await expect(p1).resolves.toBe('first');
      expect(fn).toHaveBeenCalledTimes(1);
      expect(fn).toHaveBeenCalledWith('first');

      // Advance timer to trigger trailing call
      await vi.advanceTimersByTimeAsync(100);

      expect(fn).toHaveBeenCalledTimes(2);
      expect(fn).toHaveBeenLastCalledWith('third');

      // Verify pending promises resolve with the final result
      await expect(p2).resolves.toBe('third');
      await expect(p3).resolves.toBe('third');
    });
  });

  describe('formatNumber', () => {
    it('should format numbers using Intl.NumberFormat output', () => {
      expect(formatNumber(1234567)).toBe(new Intl.NumberFormat().format(1234567));
    });

    it('should create the formatter once and reuse it', async () => {
      vi.resetModules();

      const originalNumberFormat = Intl.NumberFormat;
      const formatSpy = vi.fn((value: number) => `formatted:${value}`);
      const numberFormatSpy = vi.fn(
        () =>
          ({
            format: formatSpy,
          }) as unknown as Intl.NumberFormat,
      );

      Object.defineProperty(Intl, 'NumberFormat', {
        value: numberFormatSpy,
        configurable: true,
        writable: true,
      });

      try {
        const { formatNumber: freshFormatNumber } = await import('../utils');

        expect(freshFormatNumber(123)).toBe('formatted:123');
        expect(freshFormatNumber(456)).toBe('formatted:456');
        expect(numberFormatSpy).toHaveBeenCalledTimes(1);
        expect(formatSpy).toHaveBeenNthCalledWith(1, 123);
        expect(formatSpy).toHaveBeenNthCalledWith(2, 456);
      } finally {
        Object.defineProperty(Intl, 'NumberFormat', {
          value: originalNumberFormat,
          configurable: true,
          writable: true,
        });
        vi.resetModules();
      }
    });
  });
});

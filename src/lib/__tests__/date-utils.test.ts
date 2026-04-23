import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import {
  formatRelativeTime,
  formatSessionTimestamp,
  getDateFormatter,
} from '../date-utils';

// Lock Intl to 'en' so assertions are locale-independent
const OriginalRelativeTimeFormat = Intl.RelativeTimeFormat;
const OriginalDateTimeFormat = Intl.DateTimeFormat;

describe('date-utils', () => {
  beforeAll(() => {
    Object.defineProperty(Intl, 'RelativeTimeFormat', {
      value: class extends OriginalRelativeTimeFormat {
        constructor(
          _locale?: string | string[],
          options?: Intl.RelativeTimeFormatOptions,
        ) {
          super('en', options);
        }
      },
      writable: true,
      configurable: true,
    });

    Object.defineProperty(Intl, 'DateTimeFormat', {
      value: class extends OriginalDateTimeFormat {
        constructor(
          _locale?: string | string[],
          options?: Intl.DateTimeFormatOptions,
        ) {
          super('en-US', options);
        }
      },
      writable: true,
      configurable: true,
    });

    // Warm ICU-backed formatter initialization once so the first assertion
    // doesn't pay the full cold-start cost on slower Windows CI runners.
    new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
    new Intl.DateTimeFormat('en-US', { year: 'numeric', month: 'short' });
  });

  afterAll(() => {
    Object.defineProperty(Intl, 'RelativeTimeFormat', {
      value: OriginalRelativeTimeFormat,
      writable: true,
      configurable: true,
    });
    Object.defineProperty(Intl, 'DateTimeFormat', {
      value: OriginalDateTimeFormat,
      writable: true,
      configurable: true,
    });
  });

  describe('deterministic cache key (getDateFormatter)', () => {
    it(
      'returns the same formatter instance for options with different key insertion orders',
      () => {
        const options1: Intl.DateTimeFormatOptions = {
          year: 'numeric',
          month: 'short',
          day: 'numeric',
        };
        const options2: Intl.DateTimeFormatOptions = {
          day: 'numeric',
          year: 'numeric',
          month: 'short',
        };
        const formatter1 = getDateFormatter('en', options1);
        const formatter2 = getDateFormatter('en', options2);
        expect(formatter1).toBe(formatter2);
      },
      15000,
    );
  });

  describe('formatRelativeTime', () => {
    const reference = new Date('2023-01-01T12:00:00Z');

    it('should format seconds correctly', () => {
      const target = new Date('2023-01-01T11:59:30Z'); // 30 seconds ago
      expect(formatRelativeTime(target, reference)).toBe('30 seconds ago');
    });

    it('should format minutes correctly', () => {
      const target = new Date('2023-01-01T11:55:00Z'); // 5 minutes ago
      expect(formatRelativeTime(target, reference)).toBe('5 minutes ago');
    });

    it('should format hours correctly', () => {
      const target = new Date('2023-01-01T10:00:00Z'); // 2 hours ago
      expect(formatRelativeTime(target, reference)).toBe('2 hours ago');
    });

    it('should format days correctly', () => {
      const target = new Date('2022-12-28T12:00:00Z'); // 4 days ago
      expect(formatRelativeTime(target, reference)).toBe('4 days ago');
    });

    it('should format weeks correctly', () => {
      const target = new Date('2022-12-18T12:00:00Z'); // 2 weeks ago
      expect(formatRelativeTime(target, reference)).toBe('2 weeks ago');
    });

    it('should format months correctly', () => {
      const target = new Date('2022-10-01T12:00:00Z'); // 3 months ago
      expect(formatRelativeTime(target, reference)).toBe('3 months ago');
    });

    it('should format years correctly', () => {
      const target = new Date('2020-01-01T12:00:00Z'); // 3 years ago
      expect(formatRelativeTime(target, reference)).toBe('3 years ago');
    });

    it('should handle future dates correctly', () => {
      const target = new Date('2023-01-01T12:00:30Z'); // in 30 seconds
      expect(formatRelativeTime(target, reference)).toBe('in 30 seconds');
    });
  });

  describe('formatSessionTimestamp', () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date('2023-01-01T12:00:00Z'));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('should handle undefined date', () => {
      const result = formatSessionTimestamp(undefined);
      expect(result).toEqual({
        display: 'Unknown date',
        tooltip: 'Unknown date',
        relative: null,
      });
    });

    it('should handle invalid date string', () => {
      const result = formatSessionTimestamp('invalid-date');
      expect(result).toEqual({
        display: 'Invalid date',
        tooltip: 'Invalid date',
        relative: null,
      });
    });

    it('should format valid date object', () => {
      const date = new Date('2023-01-01T10:00:00Z'); // 2 hours ago
      const result = formatSessionTimestamp(date);

      // Note: toLocaleDateString depends on system locale, so we check loose match or mock locale
      // But standard node environment usually uses en-US or similar.
      // relative should be "2 hours ago"
      expect(result.relative).toBe('2 hours ago');
      // Verify tooltip matches the expected locale-independent format
      // Note: we use toLocaleString with explicit en-US and options to verify parity
      const expectedTooltip = date.toLocaleString('en-US', {
        year: 'numeric',
        month: 'numeric',
        day: 'numeric',
        hour: 'numeric',
        minute: 'numeric',
        second: 'numeric',
      });
      expect(result.tooltip).toBe(expectedTooltip);
      expect(result.display).toContain('2 hours ago');
    });

    it('should format valid date string', () => {
      const dateStr = '2023-01-01T10:00:00Z';
      const result = formatSessionTimestamp(dateStr);

      expect(result.relative).toBe('2 hours ago');
      expect(result.display).toContain('2 hours ago');
    });

    it('should handle date without relative time', () => {
      // Create a scenario where formatRelativeTime returns empty string.
      // The relative value is a string or null, so we test the scenario where relative is falsy.

      const dateStr = '2023-01-01T10:00:00Z';
      const date = new Date(dateStr);
      const expectedAbsolute = date.toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      });

      // Temporarily mock the global Intl.RelativeTimeFormat to return empty string
      const spy = vi.spyOn(Intl.RelativeTimeFormat.prototype, 'format').mockReturnValue('');

      try {
        const result = formatSessionTimestamp(dateStr);

        expect(result.relative).toBe('');
        // It should just return the absolute time, not the relative time
        expect(result.display).toBe(expectedAbsolute);
        expect(result.display).not.toContain('·');
      } finally {
        spy.mockRestore();
      }
    });
  });
});

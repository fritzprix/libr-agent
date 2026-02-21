import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { formatRelativeTime, formatSessionTimestamp } from '../date-utils';

describe('date-utils', () => {
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
      expect(result.tooltip).toBe(date.toLocaleString());
      expect(result.display).toContain('2 hours ago');
    });

    it('should format valid date string', () => {
      const dateStr = '2023-01-01T10:00:00Z';
      const result = formatSessionTimestamp(dateStr);

      expect(result.relative).toBe('2 hours ago');
      expect(result.display).toContain('2 hours ago');
    });
  });
});

/**
 * Date and time formatting utilities for LibrAgent
 * Provides consistent date/time formatting across the application
 */

/**
 * Formats a date relative to a reference date using Intl.RelativeTimeFormat
 * @param target The target date to format
 * @param reference The reference date (usually current time)
 * @returns Formatted relative time string (e.g., "2 hours ago", "in 3 days") or null if exceeds threshold
 */
export function formatRelativeTime(
  target: Date,
  reference: Date,
): string | null {
  const diffMs = target.getTime() - reference.getTime();
  const diffSeconds = Math.round(diffMs / 1000);

  const thresholds = [
    { limit: 60, divisor: 1, unit: 'second' as const },
    { limit: 3600, divisor: 60, unit: 'minute' as const },
    { limit: 86400, divisor: 3600, unit: 'hour' as const },
    { limit: 604800, divisor: 86400, unit: 'day' as const },
    { limit: 2629800, divisor: 604800, unit: 'week' as const },
    { limit: 31557600, divisor: 2629800, unit: 'month' as const },
  ];

  const absSeconds = Math.abs(diffSeconds);

  for (const threshold of thresholds) {
    if (absSeconds < threshold.limit) {
      const value = Math.round(diffSeconds / threshold.divisor);
      return new Intl.RelativeTimeFormat(undefined, {
        numeric: 'auto',
      }).format(value, threshold.unit);
    }
  }

  const years = Math.round(diffSeconds / 31557600);
  return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(
    years,
    'year',
  );
}

/**
 * Formats a session timestamp with both absolute and relative time
 * @param dateInput Date object, ISO string, or undefined
 * @returns Object containing display string, tooltip, and relative time
 */
export function formatSessionTimestamp(dateInput: Date | string | undefined): {
  display: string;
  tooltip: string;
  relative: string | null;
} {
  if (!dateInput) {
    return {
      display: 'Unknown date',
      tooltip: 'Unknown date',
      relative: null as string | null,
    };
  }

  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (Number.isNaN(date.getTime())) {
    return {
      display: 'Invalid date',
      tooltip: 'Invalid date',
      relative: null as string | null,
    };
  }

  const absolute = date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
  const relative = formatRelativeTime(date, new Date());

  const display = relative ? `${absolute} · ${relative}` : absolute;

  return {
    display,
    tooltip: date.toLocaleString(),
    relative,
  };
}

/**
 * Date and time formatting utilities for LibrAgent
 * Provides consistent date/time formatting across the application
 */

// Cache formatter instances to prevent expensive re-instantiations during render loops
const relativeTimeFormatters = new Map<string, Intl.RelativeTimeFormat>();
const dateFormatters = new Map<string, Intl.DateTimeFormat>();
const dateTimeFormatters = new Map<string, Intl.DateTimeFormat>();

function getCacheKey(
  locale?: string | string[],
  options?: Intl.DateTimeFormatOptions | Intl.RelativeTimeFormatOptions,
): string {
  const localeKey = locale
    ? Array.isArray(locale)
      ? locale.join(',')
      : locale
    : 'default';
  const optionsKey = options ? JSON.stringify(options) : '{}';
  return `${localeKey}-${optionsKey}`;
}

function getRelativeTimeFormatter(
  locale?: string | string[],
  options?: Intl.RelativeTimeFormatOptions,
): Intl.RelativeTimeFormat {
  const defaultOptions: Intl.RelativeTimeFormatOptions = { numeric: 'auto' };
  const finalOptions = options || defaultOptions;
  const key = getCacheKey(locale, finalOptions);

  let formatter = relativeTimeFormatters.get(key);
  if (!formatter) {
    formatter = new Intl.RelativeTimeFormat(locale, finalOptions);
    relativeTimeFormatters.set(key, formatter);
  }
  return formatter;
}

export function getDateFormatter(
  locale?: string | string[],
  options?: Intl.DateTimeFormatOptions,
): Intl.DateTimeFormat {
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  };
  const finalOptions = options || defaultOptions;
  const key = getCacheKey(locale, finalOptions);

  let formatter = dateFormatters.get(key);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(locale, finalOptions);
    dateFormatters.set(key, formatter);
  }
  return formatter;
}

export function getDateTimeFormatter(
  locale?: string | string[],
  options?: Intl.DateTimeFormatOptions,
): Intl.DateTimeFormat {
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
    hour: 'numeric',
    minute: 'numeric',
    second: 'numeric',
  };
  const finalOptions = options || defaultOptions;
  const key = getCacheKey(locale, finalOptions);

  let formatter = dateTimeFormatters.get(key);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(locale, finalOptions);
    dateTimeFormatters.set(key, formatter);
  }
  return formatter;
}

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
  const formatter = getRelativeTimeFormatter();

  for (const threshold of thresholds) {
    if (absSeconds < threshold.limit) {
      const value = Math.round(diffSeconds / threshold.divisor);
      return formatter.format(value, threshold.unit);
    }
  }

  const years = Math.round(diffSeconds / 31557600);
  return formatter.format(years, 'year');
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

  const absolute = getDateFormatter().format(date);
  const relative = formatRelativeTime(date, new Date());

  const display = relative ? `${absolute} · ${relative}` : absolute;

  return {
    display,
    tooltip: getDateTimeFormatter().format(date),
    relative,
  };
}

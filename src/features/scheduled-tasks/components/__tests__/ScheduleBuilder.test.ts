import { describe, expect, it } from 'vitest';
import {
  describeCron,
  getDisplayCron,
  isBuilderSupportedCron,
} from '../ScheduleBuilder';
import type { TFunction } from 'i18next';

const t = ((key: string, options?: { cron?: string; count?: number }) => {
  if (options?.cron) {
    return `${key}:${options.cron}`;
  }
  if (typeof options?.count === 'number') {
    return `${key}:${options.count}`;
  }
  return key;
}) as TFunction;

describe('isBuilderSupportedCron', () => {
  it('accepts simple builder expressions', () => {
    expect(isBuilderSupportedCron('*/15 * * * *')).toBe(true);
    expect(isBuilderSupportedCron('0 */4 * * *')).toBe(true);
    expect(isBuilderSupportedCron('0 9 * * *')).toBe(true);
    expect(isBuilderSupportedCron('30 9 * * 1')).toBe(true);
    expect(isBuilderSupportedCron('0 9 15 * *')).toBe(true);
  });

  it('rejects advanced expressions that would be rewritten', () => {
    expect(isBuilderSupportedCron('0 9 * * 1-5')).toBe(false);
    expect(isBuilderSupportedCron('0 9 * * 1,3,5')).toBe(false);
    expect(isBuilderSupportedCron('15 */4 * * *')).toBe(false);
    expect(isBuilderSupportedCron('0 9 * * * *')).toBe(false);
  });
});

describe('describeCron', () => {
  it('labels unsupported expressions as custom without rewriting', () => {
    expect(describeCron('0 9 * * 1-5', t)).toBe(
      'scheduledTasks.schedule.describe.custom:0 9 * * 1-5',
    );
  });
});

describe('getDisplayCron', () => {
  it('preserves unsupported cron when converting timezone display', () => {
    expect(
      getDisplayCron('0 9 * * 1-5', 'utc', Date.parse('2026-04-05T09:00:00Z')),
    ).toBe('0 9 * * 1-5');
  });
});

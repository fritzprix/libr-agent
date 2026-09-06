import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import type { ScheduledTaskTimezone } from '@/lib/backend/scheduled-tasks';

type RepeatMode = 'minutes' | 'hours' | 'daily' | 'weekly' | 'monthly';

interface ScheduleState {
  mode: RepeatMode;
  minuteInterval: number; // every N minutes
  hourInterval: number; // every N hours
  hour: number; // 0-23
  minute: number; // 0-59
  weekDay: number; // 0=Sun … 6=Sat
  monthDay: number; // 1-31
}

const DEFAULT_STATE: ScheduleState = {
  mode: 'daily',
  minuteInterval: 15,
  hourInterval: 4,
  hour: 9,
  minute: 0,
  weekDay: 1,
  monthDay: 1,
};

const WEEK_DAYS = [0, 1, 2, 3, 4, 5, 6];

function toCron(s: ScheduleState): string {
  switch (s.mode) {
    case 'minutes':
      return `*/${s.minuteInterval} * * * *`;
    case 'hours':
      return `0 */${s.hourInterval} * * *`;
    case 'daily':
      return `${s.minute} ${s.hour} * * *`;
    case 'weekly':
      return `${s.minute} ${s.hour} * * ${s.weekDay}`;
    case 'monthly':
      return `${s.minute} ${s.hour} ${s.monthDay} * *`;
  }
}

function fromCron(cron: string): ScheduleState {
  const parts = cron.trim().split(/\s+/);
  if (parts.length !== 5) return DEFAULT_STATE;

  const [min, hour, dom, , dow] = parts;

  if (min.startsWith('*/') && hour === '*') {
    return {
      ...DEFAULT_STATE,
      mode: 'minutes',
      minuteInterval: parseInt(min.slice(2)) || 15,
    };
  }
  if (min === '0' && hour.startsWith('*/')) {
    return {
      ...DEFAULT_STATE,
      mode: 'hours',
      hourInterval: parseInt(hour.slice(2)) || 4,
    };
  }
  if (dom !== '*' && dow === '*') {
    return {
      ...DEFAULT_STATE,
      mode: 'monthly',
      minute: parseInt(min) || 0,
      hour: parseInt(hour) || 9,
      monthDay: parseInt(dom) || 1,
    };
  }
  if (dom === '*' && dow !== '*') {
    return {
      ...DEFAULT_STATE,
      mode: 'weekly',
      minute: parseInt(min) || 0,
      hour: parseInt(hour) || 9,
      weekDay: parseInt(dow) || 1,
    };
  }
  return {
    ...DEFAULT_STATE,
    mode: 'daily',
    minute: parseInt(min) || 0,
    hour: parseInt(hour) || 9,
  };
}

/**
 * True when the simple builder can round-trip `cron` without changing it.
 * Unsupported expressions (ranges, lists, etc.) must stay in custom mode.
 */
export function isBuilderSupportedCron(cron: string): boolean {
  const trimmed = cron.trim();
  if (!trimmed) {
    return true;
  }
  const parts = trimmed.split(/\s+/);
  if (parts.length !== 5) {
    return false;
  }
  return toCron(fromCron(trimmed)) === trimmed;
}

/** Format a ScheduleState into a human-readable summary string. */
export function describeCron(cron: string, t: TFunction): string {
  const trimmed = cron.trim();
  if (!isBuilderSupportedCron(trimmed)) {
    return t('scheduledTasks.schedule.describe.custom', {
      cron: trimmed,
      defaultValue: 'Custom: {{cron}}',
    });
  }

  const s = fromCron(trimmed);
  const timeStr = `${String(s.hour).padStart(2, '0')}:${String(s.minute).padStart(2, '0')}`;
  switch (s.mode) {
    case 'minutes':
      return t('scheduledTasks.schedule.describe.minutes', {
        count: s.minuteInterval,
      });
    case 'hours':
      return t('scheduledTasks.schedule.describe.hours', {
        count: s.hourInterval,
      });
    case 'daily':
      return t('scheduledTasks.schedule.describe.daily', { time: timeStr });
    case 'weekly':
      return t('scheduledTasks.schedule.describe.weekly', {
        day: t(`scheduledTasks.schedule.days.${s.weekDay}`),
        time: timeStr,
      });
    case 'monthly':
      return t('scheduledTasks.schedule.describe.monthly', {
        day: s.monthDay,
        time: timeStr,
      });
  }
}

export function getDisplayCron(
  cron: string,
  scheduleTimezone: ScheduledTaskTimezone,
  nextRunAt: number | null,
): string {
  if (scheduleTimezone === 'local' || nextRunAt === null) {
    return cron;
  }

  // Do not rewrite unsupported expressions via timezone display conversion.
  if (!isBuilderSupportedCron(cron)) {
    return cron;
  }

  const state = fromCron(cron);
  if (state.mode === 'minutes' || state.mode === 'hours') {
    return cron;
  }

  const nextRun = new Date(nextRunAt);
  if (Number.isNaN(nextRun.getTime())) {
    return cron;
  }

  switch (state.mode) {
    case 'daily':
      return `${nextRun.getMinutes()} ${nextRun.getHours()} * * *`;
    case 'weekly':
      return `${nextRun.getMinutes()} ${nextRun.getHours()} * * ${nextRun.getDay()}`;
    case 'monthly':
      return `${nextRun.getMinutes()} ${nextRun.getHours()} ${nextRun.getDate()} * *`;
  }
}

interface ScheduleBuilderProps {
  value: string; // cron expression
  onChange: (cron: string) => void;
}

/**
 * Human-readable schedule builder that produces a cron expression.
 * Unsupported (advanced) expressions stay editable as raw cron so they are
 * never silently rewritten to the daily default.
 */
export function ScheduleBuilder({ value, onChange }: ScheduleBuilderProps) {
  const { t } = useTranslation();
  const isCustom = useMemo(() => !isBuilderSupportedCron(value), [value]);
  const state = useMemo(() => fromCron(value), [value]);

  const update = useCallback(
    (patch: Partial<ScheduleState>) => {
      const next = { ...state, ...patch };
      onChange(toCron(next));
    },
    [state, onChange],
  );

  const clamp = (v: number, min: number, max: number) =>
    Math.max(min, Math.min(max, v));

  const showsTimeField =
    state.mode === 'daily' ||
    state.mode === 'weekly' ||
    state.mode === 'monthly';

  if (isCustom) {
    return (
      <div className="flex flex-col gap-3">
        <p className="text-sm text-muted-foreground">
          {t(
            'scheduledTasks.schedule.customHint',
            'This schedule uses an advanced cron expression the simple editor cannot represent. Edit the expression below, or switch to a simple schedule (this replaces the current expression).',
          )}
        </p>
        <div className="flex flex-col gap-2">
          <Label htmlFor="schedule-custom-cron">
            {t('scheduledTasks.schedule.customLabel', 'Cron expression')}
          </Label>
          <Input
            id="schedule-custom-cron"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            spellCheck={false}
            className="font-mono text-sm"
            aria-label={t(
              'scheduledTasks.schedule.customLabel',
              'Cron expression',
            )}
          />
        </div>
        <p className="text-xs text-muted-foreground">
          {t('scheduledTasks.schedule.summary')}
          <span className="font-medium text-foreground">
            {describeCron(value, t)}
          </span>
        </p>
        <div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => onChange(toCron(DEFAULT_STATE))}
          >
            {t(
              'scheduledTasks.schedule.switchToSimple',
              'Switch to simple schedule',
            )}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
        <div className="flex min-w-0 flex-1 items-center gap-2 sm:min-w-[12rem]">
          <Label className="shrink-0">
            {t('scheduledTasks.schedule.repeat')}
          </Label>
          <Select
            value={state.mode}
            onValueChange={(v) => update({ mode: v as RepeatMode })}
          >
            <SelectTrigger className="min-w-0 flex-1">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="minutes">
                {t('scheduledTasks.schedule.modes.minutes')}
              </SelectItem>
              <SelectItem value="hours">
                {t('scheduledTasks.schedule.modes.hours')}
              </SelectItem>
              <SelectItem value="daily">
                {t('scheduledTasks.schedule.modes.daily')}
              </SelectItem>
              <SelectItem value="weekly">
                {t('scheduledTasks.schedule.modes.weekly')}
              </SelectItem>
              <SelectItem value="monthly">
                {t('scheduledTasks.schedule.modes.monthly')}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        {showsTimeField ? (
          <div className="flex items-center gap-2">
            <Label className="shrink-0">
              {t('scheduledTasks.schedule.at')}
            </Label>
            <div className="flex items-center gap-1.5">
              <Input
                type="number"
                min={0}
                max={23}
                value={state.hour}
                onChange={(e) =>
                  update({
                    hour: clamp(parseInt(e.target.value) || 0, 0, 23),
                  })
                }
                className="w-16 text-center"
                aria-label={t('scheduledTasks.schedule.hour')}
              />
              <span className="font-semibold text-muted-foreground">:</span>
              <Input
                type="number"
                min={0}
                max={59}
                value={String(state.minute).padStart(2, '0')}
                onChange={(e) =>
                  update({
                    minute: clamp(parseInt(e.target.value) || 0, 0, 59),
                  })
                }
                className="w-16 text-center"
                aria-label={t('scheduledTasks.schedule.minute')}
              />
            </div>
          </div>
        ) : null}
      </div>

      {state.mode === 'minutes' && (
        <div className="flex items-center gap-2">
          <Label className="shrink-0">
            {t('scheduledTasks.schedule.every')}
          </Label>
          <Input
            type="number"
            min={1}
            max={59}
            value={state.minuteInterval}
            onChange={(e) =>
              update({
                minuteInterval: clamp(parseInt(e.target.value) || 15, 1, 59),
              })
            }
            className="w-20"
          />
          <span className="text-sm text-muted-foreground">
            {t('scheduledTasks.schedule.units.minutes')}
          </span>
        </div>
      )}

      {state.mode === 'hours' && (
        <div className="flex items-center gap-2">
          <Label className="shrink-0">
            {t('scheduledTasks.schedule.every')}
          </Label>
          <Input
            type="number"
            min={1}
            max={23}
            value={state.hourInterval}
            onChange={(e) =>
              update({
                hourInterval: clamp(parseInt(e.target.value) || 4, 1, 23),
              })
            }
            className="w-20"
          />
          <span className="text-sm text-muted-foreground">
            {t('scheduledTasks.schedule.units.hours')}
          </span>
        </div>
      )}

      {state.mode === 'weekly' && (
        <div className="flex min-w-0 items-center gap-2">
          <Label className="shrink-0">{t('scheduledTasks.schedule.on')}</Label>
          <Select
            value={String(state.weekDay)}
            onValueChange={(v) => update({ weekDay: parseInt(v) })}
          >
            <SelectTrigger className="min-w-0 flex-1">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {WEEK_DAYS.map((dayIndex) => (
                <SelectItem key={dayIndex} value={String(dayIndex)}>
                  {t(`scheduledTasks.schedule.days.${dayIndex}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}

      {state.mode === 'monthly' && (
        <div className="flex items-center gap-2">
          <Label className="shrink-0">
            {t('scheduledTasks.schedule.onDay')}
          </Label>
          <Input
            type="number"
            min={1}
            max={31}
            value={state.monthDay}
            onChange={(e) =>
              update({ monthDay: clamp(parseInt(e.target.value) || 1, 1, 31) })
            }
            className="w-20"
          />
        </div>
      )}

      <p className="text-xs text-muted-foreground">
        {t('scheduledTasks.schedule.summary')}
        <span className="font-medium text-foreground">
          {describeCron(toCron(state), t)}
        </span>
      </p>
    </div>
  );
}

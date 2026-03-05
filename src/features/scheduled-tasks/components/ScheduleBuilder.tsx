import { useCallback, useMemo } from 'react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

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

const WEEK_DAYS = [
  'Sunday',
  'Monday',
  'Tuesday',
  'Wednesday',
  'Thursday',
  'Friday',
  'Saturday',
];

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

/** Format a ScheduleState into a human-readable summary string. */
export function describeCron(cron: string): string {
  const s = fromCron(cron);
  const timeStr = `${String(s.hour).padStart(2, '0')}:${String(s.minute).padStart(2, '0')}`;
  switch (s.mode) {
    case 'minutes':
      return `Every ${s.minuteInterval} minute${s.minuteInterval === 1 ? '' : 's'}`;
    case 'hours':
      return `Every ${s.hourInterval} hour${s.hourInterval === 1 ? '' : 's'}`;
    case 'daily':
      return `Daily at ${timeStr}`;
    case 'weekly':
      return `Every ${WEEK_DAYS[s.weekDay]} at ${timeStr}`;
    case 'monthly':
      return `Monthly on day ${s.monthDay} at ${timeStr}`;
  }
}

interface ScheduleBuilderProps {
  value: string; // cron expression
  onChange: (cron: string) => void;
}

/**
 * Human-readable schedule builder that produces a cron expression.
 * Users never need to see or type cron syntax.
 */
export function ScheduleBuilder({ value, onChange }: ScheduleBuilderProps) {
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

  return (
    <div className="grid gap-3">
      {/* Repeat mode */}
      <div className="grid gap-1.5">
        <Label>Repeat</Label>
        <Select
          value={state.mode}
          onValueChange={(v) => update({ mode: v as RepeatMode })}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="minutes">Every N minutes</SelectItem>
            <SelectItem value="hours">Every N hours</SelectItem>
            <SelectItem value="daily">Daily</SelectItem>
            <SelectItem value="weekly">Weekly</SelectItem>
            <SelectItem value="monthly">Monthly</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Every N minutes */}
      {state.mode === 'minutes' && (
        <div className="grid gap-1.5">
          <Label>Every</Label>
          <div className="flex items-center gap-2">
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
            <span className="text-sm text-muted-foreground">minutes</span>
          </div>
        </div>
      )}

      {/* Every N hours */}
      {state.mode === 'hours' && (
        <div className="grid gap-1.5">
          <Label>Every</Label>
          <div className="flex items-center gap-2">
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
            <span className="text-sm text-muted-foreground">hours</span>
          </div>
        </div>
      )}

      {/* Day of week (weekly only) */}
      {state.mode === 'weekly' && (
        <div className="grid gap-1.5">
          <Label>On</Label>
          <Select
            value={String(state.weekDay)}
            onValueChange={(v) => update({ weekDay: parseInt(v) })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {WEEK_DAYS.map((day, i) => (
                <SelectItem key={day} value={String(i)}>
                  {day}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}

      {/* Day of month (monthly only) */}
      {state.mode === 'monthly' && (
        <div className="grid gap-1.5">
          <Label>On day</Label>
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

      {/* Time (daily / weekly / monthly) */}
      {(state.mode === 'daily' ||
        state.mode === 'weekly' ||
        state.mode === 'monthly') && (
        <div className="grid gap-1.5">
          <Label>At</Label>
          <div className="flex items-center gap-1.5">
            <Input
              type="number"
              min={0}
              max={23}
              value={state.hour}
              onChange={(e) =>
                update({ hour: clamp(parseInt(e.target.value) || 0, 0, 23) })
              }
              className="w-16 text-center"
              aria-label="Hour"
            />
            <span className="text-muted-foreground font-semibold">:</span>
            <Input
              type="number"
              min={0}
              max={59}
              value={String(state.minute).padStart(2, '0')}
              onChange={(e) =>
                update({ minute: clamp(parseInt(e.target.value) || 0, 0, 59) })
              }
              className="w-16 text-center"
              aria-label="Minute"
            />
          </div>
        </div>
      )}

      {/* Live summary */}
      <p className="text-xs text-muted-foreground">
        Schedule:{' '}
        <span className="font-medium text-foreground">
          {describeCron(toCron(state))}
        </span>
      </p>
    </div>
  );
}

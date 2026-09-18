import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Clock, Loader2, Timer, X } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import { getDateTimeFormatter } from '@/lib/date-utils';
import type { SessionScheduledTask } from '@/lib/backend/scheduled-tasks';
import { cn } from '@/lib/utils';
import { useSessionSchedules } from '../hooks/useSessionSchedules';
import { toast } from 'sonner';

interface SessionSchedulesSectionProps {
  sessionId: string;
  isVisible?: boolean;
}

function formatCountdown(nextRunAt: number | null): string {
  if (nextRunAt === null) {
    return '--:--';
  }

  const diffMs = nextRunAt - Date.now();
  if (diffMs <= 0) {
    return '0:00';
  }

  const totalSeconds = Math.ceil(diffMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

export function SessionSchedulesSection({
  sessionId,
  isVisible = true,
}: SessionSchedulesSectionProps) {
  const { t } = useTranslation();
  const { tasks, loading, cancellingIds, togglingIds, cancelTask, toggleTask } =
    useSessionSchedules(sessionId, isVisible);
  const [, setTick] = useState(0);

  const hasActiveOneShot = useMemo(
    () => tasks.some((task) => task.isOneShot && task.enabled),
    [tasks],
  );

  useEffect(() => {
    if (!isVisible || !hasActiveOneShot) {
      return;
    }

    const timer = window.setInterval(() => {
      setTick((value) => value + 1);
    }, 1000);

    return () => window.clearInterval(timer);
  }, [hasActiveOneShot, isVisible]);

  const formatNextRun = (nextRunAt: number | null): string => {
    if (!nextRunAt) {
      return t('agent.planning.schedules.nextRunNone', 'Not scheduled');
    }

    return getDateTimeFormatter().format(new Date(nextRunAt));
  };

  const handleCancel = async (taskId: string) => {
    try {
      await cancelTask(taskId);
    } catch {
      toast.error(
        t(
          'agent.planning.schedules.cancelFailed',
          'Failed to cancel scheduled callback',
        ),
      );
    }
  };

  const handleToggle = async (task: SessionScheduledTask) => {
    try {
      await toggleTask(task);
    } catch {
      toast.error(
        t(
          'agent.planning.schedules.toggleFailed',
          'Failed to update scheduled callback',
        ),
      );
    }
  };

  return (
    <section className="flex shrink-0 flex-col space-y-2 border-t border-border/40 pt-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Timer className="h-3.5 w-3.5 text-muted-foreground" />
          <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
            {t('agent.planning.schedules.title', 'Schedules')}
          </h4>
        </div>
        <span className="text-[11px] text-muted-foreground">
          {tasks.length}
        </span>
      </div>

      <div className="max-h-40 overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]">
        <ScrollArea className="h-full max-h-40">
          {loading ? (
            <div className="flex items-center gap-2 px-3 py-4 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t('common.loading', 'Loading...')}
            </div>
          ) : tasks.length === 0 ? (
            <div className="px-3 py-4 text-sm text-muted-foreground">
              {t(
                'agent.planning.schedules.empty',
                'No session callbacks scheduled',
              )}
            </div>
          ) : (
            <div>
              {tasks.map((task) => (
                <div
                  key={task.id}
                  className={cn(
                    'border-b border-border/25 px-3 py-3 last:border-b-0',
                    !task.enabled && 'opacity-70',
                  )}
                >
                  <div className="flex items-start gap-2">
                    <Switch
                      checked={task.enabled}
                      onCheckedChange={() => void handleToggle(task)}
                      className="mt-0.5 shrink-0"
                      disabled={
                        togglingIds.has(task.id) || cancellingIds.has(task.id)
                      }
                      aria-label={
                        task.enabled
                          ? t(
                              'agent.planning.schedules.pauseAria',
                              'Pause {{name}}',
                              { name: task.name },
                            )
                          : t(
                              'agent.planning.schedules.resumeAria',
                              'Resume {{name}}',
                              { name: task.name },
                            )
                      }
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm font-medium text-foreground">
                          {task.name}
                        </span>
                        {task.isOneShot ? (
                          <Badge variant="secondary" className="text-[10px]">
                            {t('agent.planning.schedules.oneShot', 'One-shot')}
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-[10px]">
                            {t(
                              'agent.planning.schedules.recurring',
                              'Recurring',
                            )}
                          </Badge>
                        )}
                        {task.enabled ? null : (
                          <Badge variant="outline" className="text-[10px]">
                            {t('agent.planning.schedules.paused', 'Paused')}
                          </Badge>
                        )}
                      </div>
                      <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                        {task.message}
                      </p>
                      <div className="mt-2 flex items-center gap-1.5 text-xs text-muted-foreground">
                        {task.isOneShot ? (
                          <>
                            <Timer className="h-3.5 w-3.5 shrink-0" />
                            <span>
                              {t(
                                'agent.planning.schedules.remaining',
                                '{{time}} remaining',
                                { time: formatCountdown(task.nextRunAt) },
                              )}
                            </span>
                          </>
                        ) : (
                          <>
                            <Clock className="h-3.5 w-3.5 shrink-0" />
                            <span>
                              {t(
                                'agent.planning.schedules.nextRun',
                                'Next run: {{time}}',
                                { time: formatNextRun(task.nextRunAt) },
                              )}
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
                      onClick={() => void handleCancel(task.id)}
                      disabled={
                        cancellingIds.has(task.id) || togglingIds.has(task.id)
                      }
                      aria-label={t(
                        'agent.planning.schedules.cancelAria',
                        'Cancel {{name}}',
                        { name: task.name },
                      )}
                    >
                      {cancellingIds.has(task.id) ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <X className="h-3.5 w-3.5" />
                      )}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </ScrollArea>
      </div>
    </section>
  );
}

import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Clock,
  FolderOpen,
  Layers3,
  Loader2,
  Pencil,
  Trash2,
  Zap,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { describeCron, getDisplayCron } from './ScheduleBuilder';
import type { ScheduledTaskGroupSection } from '../scheduled-task-utils';

interface ScheduledTaskRowProps {
  task: ScheduledTask;
  formatNextRun: (ms: number | null) => string;
  onEdit: (task: ScheduledTask) => void;
  onToggle: (task: ScheduledTask) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  isToggling: boolean;
  isDeleting: boolean;
}

export const ScheduledTaskRow = memo(function ScheduledTaskRow({
  task,
  formatNextRun,
  onEdit,
  onToggle,
  onDelete,
  isToggling,
  isDeleting,
}: ScheduledTaskRowProps) {
  const { t } = useTranslation();

  return (
    <li className="flex items-start gap-4 rounded-lg border border-border bg-card p-4">
      <Switch
        checked={task.enabled}
        onCheckedChange={() => void onToggle(task)}
        className="mt-0.5 shrink-0"
        disabled={isToggling || isDeleting}
      />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="truncate font-medium">{task.name}</span>
          <Badge variant="secondary" className="shrink-0 text-xs">
            {describeCron(
              getDisplayCron(
                task.cronExpression,
                task.scheduleTimezone,
                task.nextRunAt,
              ),
              t,
            )}
          </Badge>
          {task.scheduleTimezone === 'utc' && (
            <Badge variant="outline" className="shrink-0 text-xs">
              {t('scheduledTasks.utcLegacy', 'UTC legacy')}
            </Badge>
          )}
          {task.yoloMode && (
            <Badge
              variant="default"
              className="shrink-0 bg-primary/80 text-xs hover:bg-primary/80"
            >
              <Zap size={10} className="mr-1 fill-current" />
              YOLO
            </Badge>
          )}
          {task.groupName && (
            <Badge variant="outline" className="shrink-0 text-xs">
              {t('scheduledTasks.groupBadge', {
                name: task.groupName,
                defaultValue: 'Group: {{name}}',
              })}
            </Badge>
          )}
          {!task.enabled && (
            <Badge variant="outline" className="shrink-0 text-xs">
              {t('scheduledTasks.disabled')}
            </Badge>
          )}
        </div>
        <p className="mt-1 line-clamp-2 break-all text-sm text-muted-foreground">
          {task.message}
        </p>
        <p className="mt-1 text-xs text-muted-foreground">
          {t('scheduledTasks.nextRun', {
            time: formatNextRun(task.nextRunAt),
          })}
        </p>
        {task.workspaceOverride && (
          <div className="mt-2 flex items-center gap-1.5 text-xs text-muted-foreground">
            <FolderOpen className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate" title={task.workspaceOverride}>
              {task.workspaceOverride}
            </span>
          </div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => onEdit(task)}
              aria-label={t('scheduledTasks.editTaskAria', {
                name: task.name,
              })}
            >
              <Pencil className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t('scheduledTasks.editTask')}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-destructive hover:text-destructive"
              onClick={() => void onDelete(task.id)}
              aria-label={t('scheduledTasks.deleteTaskAria', {
                name: task.name,
              })}
              disabled={isDeleting || isToggling}
            >
              {isDeleting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t('scheduledTasks.deleteTask')}</TooltipContent>
        </Tooltip>
      </div>
    </li>
  );
});

interface SummaryCardProps {
  title: string;
  value: number;
  description: string;
}

function SummaryCard({ title, value, description }: SummaryCardProps) {
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="pb-0">
        <CardDescription>{title}</CardDescription>
        <CardTitle className="text-2xl">{value}</CardTitle>
      </CardHeader>
      <CardContent className="pt-0 text-xs text-muted-foreground">
        {description}
      </CardContent>
    </Card>
  );
}

interface ScheduledTasksContentProps {
  enabledTaskCount: number;
  formatNextRun: (ms: number | null) => string;
  groupedSections: ScheduledTaskGroupSection[];
  onCreate: () => void;
  onDelete: (id: string) => Promise<void>;
  onEdit: (task: ScheduledTask) => void;
  onToggle: (task: ScheduledTask) => Promise<void>;
  personalTasks: ScheduledTask[];
  tasks: ScheduledTask[];
  deletingIds: Set<string>;
  togglingIds: Set<string>;
}

export function ScheduledTasksContent({
  enabledTaskCount,
  formatNextRun,
  groupedSections,
  onCreate,
  onDelete,
  onEdit,
  onToggle,
  personalTasks,
  tasks,
  deletingIds,
  togglingIds,
}: ScheduledTasksContentProps) {
  const { t } = useTranslation();

  if (tasks.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed py-16 text-muted-foreground">
        <Clock className="h-8 w-8 opacity-40" />
        <p className="text-sm">{t('scheduledTasks.noTasks')}</p>
        <Button variant="outline" size="sm" onClick={onCreate}>
          {t('scheduledTasks.createFirst')}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-3 md:grid-cols-3">
        <SummaryCard
          title={t('scheduledTasks.summary.totalTasks', 'Total Tasks')}
          value={tasks.length}
          description={t(
            'scheduledTasks.summary.totalTasksDescription',
            'All recurring runs currently configured.',
          )}
        />
        <SummaryCard
          title={t('scheduledTasks.summary.taskGroups', 'Task Groups')}
          value={groupedSections.length}
          description={t(
            'scheduledTasks.summary.taskGroupsDescription',
            'Grouped automation teams sharing a recurring schedule surface.',
          )}
        />
        <SummaryCard
          title={t('scheduledTasks.summary.enabledTasks', 'Enabled Tasks')}
          value={enabledTaskCount}
          description={t(
            'scheduledTasks.summary.enabledTasksDescription',
            'Currently active recurring runs.',
          )}
        />
      </div>

      {groupedSections.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-center gap-2">
            <Layers3 className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">
              {t('scheduledTasks.groupsTitle', 'Scheduled Task Groups')}
            </h2>
          </div>
          {groupedSections.map((group) => {
            const nextGroupRun =
              group.tasks.find(
                (task) => task.enabled && task.nextRunAt !== null,
              )?.nextRunAt ?? null;

            return (
              <Card key={group.key} className="gap-4 py-4">
                <CardHeader className="gap-2 pb-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <CardTitle>{group.groupName}</CardTitle>
                    <Badge variant="secondary" className="text-xs">
                      {t('scheduledTasks.groupTaskCount', {
                        count: group.tasks.length,
                        defaultValue: '{{count}} tasks',
                      })}
                    </Badge>
                    <Badge variant="outline" className="text-xs">
                      {t('scheduledTasks.groupEnabledCount', {
                        count: group.enabledCount,
                        defaultValue: '{{count}} enabled',
                      })}
                    </Badge>
                  </div>
                  <CardDescription>
                    {t('scheduledTasks.groupNextRun', {
                      time: formatNextRun(nextGroupRun),
                      defaultValue: 'Next group run: {{time}}',
                    })}
                  </CardDescription>
                </CardHeader>
                <CardContent className="px-4">
                  <ul className="flex flex-col gap-3">
                    {group.tasks.map((task) => (
                      <ScheduledTaskRow
                        key={task.id}
                        task={task}
                        formatNextRun={formatNextRun}
                        onEdit={onEdit}
                        onToggle={onToggle}
                        onDelete={onDelete}
                        isToggling={togglingIds.has(task.id)}
                        isDeleting={deletingIds.has(task.id)}
                      />
                    ))}
                  </ul>
                </CardContent>
              </Card>
            );
          })}
        </section>
      )}

      {personalTasks.length > 0 && (
        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-semibold">
            {t('scheduledTasks.personalTitle', 'Standalone Tasks')}
          </h2>
          <ul className="flex flex-col gap-3">
            {personalTasks.map((task) => (
              <ScheduledTaskRow
                key={task.id}
                task={task}
                formatNextRun={formatNextRun}
                onEdit={onEdit}
                onToggle={onToggle}
                onDelete={onDelete}
                isToggling={togglingIds.has(task.id)}
                isDeleting={deletingIds.has(task.id)}
              />
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

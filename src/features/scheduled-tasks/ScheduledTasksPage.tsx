import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  Plus,
  Pencil,
  Trash2,
  Clock,
  Zap,
  FolderOpen,
  Loader2,
  Layers3,
} from 'lucide-react';
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { ScheduledTaskModal } from './components/ScheduledTaskModal';
import { describeCron, getDisplayCron } from './components/ScheduleBuilder';
import { useScheduledTasks } from './hooks/useScheduledTasks';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useAssistantContext } from '@/context/AssistantContext';
import { getDateTimeFormatter } from '@/lib/date-utils';

const logger = getLogger('ScheduledTasksPage');

interface ScheduledTaskFormData {
  name: string;
  cronExpression: string;
  scheduleTimezone: 'local';
  assistantId: string;
  groupName: string | null;
  message: string;
  yoloMode: boolean;
  workspaceOverride: string | null;
  clearGroup?: boolean;
}

interface ScheduledTaskGroupSection {
  key: string;
  groupId: string | null;
  groupName: string;
  tasks: ScheduledTask[];
}

export function ScheduledTasksPage() {
  const { t } = useTranslation();

  const {
    tasks,
    loading,
    togglingIds,
    deletingIds,
    createTask,
    updateTask,
    toggleTask,
    deleteTask,
  } = useScheduledTasks();

  const { assistants } = useAssistantContext();

  const [modalOpen, setModalOpen] = useState(false);
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);

  const handleCreate = async (data: ScheduledTaskFormData) => {
    await createTask(data);
  };

  const handleUpdate = async (data: ScheduledTaskFormData) => {
    if (!editingTask) return;
    await updateTask(editingTask.id, data);
  };

  const handleToggle = async (task: ScheduledTask) => {
    try {
      await toggleTask(task);
    } catch (error) {
      logger.error('Failed to toggle scheduled task', error);
      toast.error(
        t('scheduledTasks.toggleFailed', 'Failed to update scheduled task'),
      );
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteTask(id);
    } catch (error) {
      logger.error('Failed to delete scheduled task', error);
      toast.error(
        t('scheduledTasks.deleteFailed', 'Failed to delete scheduled task'),
      );
    }
  };

  const openCreate = () => {
    setEditingTask(null);
    setModalOpen(true);
  };

  const openEdit = (task: ScheduledTask) => {
    setEditingTask(task);
    setModalOpen(true);
  };

  const formatNextRun = (ms: number | null): string => {
    if (!ms) return t('scheduledTasks.nextRunNone');
    const d = new Date(ms);
    return getDateTimeFormatter().format(d);
  };

  const groupedSections = useMemo<ScheduledTaskGroupSection[]>(() => {
    const groups = new Map<string, ScheduledTaskGroupSection>();

    for (const task of tasks) {
      if (!task.groupName) {
        continue;
      }

      const key = task.groupId ?? `group:${task.groupName}`;
      const existing = groups.get(key);
      if (existing) {
        existing.tasks.push(task);
        continue;
      }

      groups.set(key, {
        key,
        groupId: task.groupId,
        groupName: task.groupName,
        tasks: [task],
      });
    }

    return Array.from(groups.values())
      .map((group) => ({
        ...group,
        tasks: [...group.tasks].sort(compareScheduledTasks),
      }))
      .sort((left, right) => left.groupName.localeCompare(right.groupName));
  }, [tasks]);

  const personalTasks = useMemo(
    () => tasks.filter((task) => !task.groupName).sort(compareScheduledTasks),
    [tasks],
  );

  const enabledTaskCount = tasks.filter((task) => task.enabled).length;

  if (loading) {
    return (
      <div
        className="flex flex-col gap-6 p-6 max-w-3xl mx-auto"
        role="status"
        aria-busy="true"
      >
        <span className="sr-only">{t('common.loading')}</span>
        <div className="flex items-center justify-between" aria-hidden="true">
          <div>
            <Skeleton className="h-7 w-48 mb-1" />
            <Skeleton className="h-4 w-64" />
          </div>
          <Skeleton className="h-9 w-24" />
        </div>
        <div className="flex flex-col gap-3" aria-hidden="true">
          <Skeleton className="h-24 w-full rounded-lg" />
          <Skeleton className="h-24 w-full rounded-lg" />
          <Skeleton className="h-24 w-full rounded-lg" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6 max-w-3xl mx-auto">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">{t('scheduledTasks.title')}</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            {t('scheduledTasks.subtitle')}
          </p>
        </div>
        <Button onClick={openCreate} size="sm">
          <Plus className="w-4 h-4 mr-1" />
          {t('scheduledTasks.newTask')}
        </Button>
      </div>

      {tasks.length > 0 && (
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
      )}

      {tasks.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-16 border border-dashed rounded-lg text-muted-foreground">
          <Clock className="w-8 h-8 opacity-40" />
          <p className="text-sm">{t('scheduledTasks.noTasks')}</p>
          <Button variant="outline" size="sm" onClick={openCreate}>
            {t('scheduledTasks.createFirst')}
          </Button>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          {groupedSections.length > 0 && (
            <section className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                <Layers3 className="h-4 w-4 text-muted-foreground" />
                <h2 className="text-sm font-semibold">
                  {t('scheduledTasks.groupsTitle', 'Scheduled Task Groups')}
                </h2>
              </div>
              {groupedSections.map((group) => {
                const groupEnabledCount = group.tasks.filter(
                  (task) => task.enabled,
                ).length;
                const nextGroupRun =
                  group.tasks
                    .filter((task) => task.enabled && task.nextRunAt !== null)
                    .sort((left, right) => {
                      if (left.nextRunAt === null) return 1;
                      if (right.nextRunAt === null) return -1;
                      return left.nextRunAt - right.nextRunAt;
                    })[0]?.nextRunAt ?? null;

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
                            count: groupEnabledCount,
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
                            onEdit={openEdit}
                            onToggle={handleToggle}
                            onDelete={handleDelete}
                            togglingIds={togglingIds}
                            deletingIds={deletingIds}
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
                    onEdit={openEdit}
                    onToggle={handleToggle}
                    onDelete={handleDelete}
                    togglingIds={togglingIds}
                    deletingIds={deletingIds}
                  />
                ))}
              </ul>
            </section>
          )}
        </div>
      )}

      <ScheduledTaskModal
        open={modalOpen}
        task={editingTask}
        assistants={assistants}
        onClose={() => setModalOpen(false)}
        onSave={editingTask ? handleUpdate : handleCreate}
      />
    </div>
  );
}

function SummaryCard({
  title,
  value,
  description,
}: {
  title: string;
  value: number;
  description: string;
}) {
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

function ScheduledTaskRow({
  task,
  formatNextRun,
  onEdit,
  onToggle,
  onDelete,
  togglingIds,
  deletingIds,
}: {
  task: ScheduledTask;
  formatNextRun: (ms: number | null) => string;
  onEdit: (task: ScheduledTask) => void;
  onToggle: (task: ScheduledTask) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  togglingIds: Set<string>;
  deletingIds: Set<string>;
}) {
  const { t } = useTranslation();

  return (
    <li className="flex items-start gap-4 rounded-lg border border-border p-4 bg-card">
      <Switch
        checked={task.enabled}
        onCheckedChange={() => void onToggle(task)}
        className="mt-0.5 shrink-0"
        disabled={togglingIds.has(task.id) || deletingIds.has(task.id)}
      />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-medium truncate">{task.name}</span>
          <Badge variant="secondary" className="text-xs shrink-0">
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
            <Badge variant="outline" className="text-xs shrink-0">
              {t('scheduledTasks.utcLegacy', 'UTC legacy')}
            </Badge>
          )}
          {task.yoloMode && (
            <Badge
              variant="default"
              className="text-xs shrink-0 bg-primary/80 hover:bg-primary/80"
            >
              <Zap size={10} className="mr-1 fill-current" />
              YOLO
            </Badge>
          )}
          {task.groupName && (
            <Badge variant="outline" className="text-xs shrink-0">
              {t('scheduledTasks.groupBadge', {
                name: task.groupName,
                defaultValue: 'Group: {{name}}',
              })}
            </Badge>
          )}
          {!task.enabled && (
            <Badge variant="outline" className="text-xs shrink-0">
              {t('scheduledTasks.disabled')}
            </Badge>
          )}
        </div>
        <p className="text-sm text-muted-foreground mt-1 line-clamp-2 break-all">
          {task.message}
        </p>
        <p className="text-xs text-muted-foreground mt-1">
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
      <div className="flex items-center gap-1 shrink-0">
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
              <Pencil className="w-4 h-4" />
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
              disabled={deletingIds.has(task.id) || togglingIds.has(task.id)}
            >
              {deletingIds.has(task.id) ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Trash2 className="w-4 h-4" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t('scheduledTasks.deleteTask')}</TooltipContent>
        </Tooltip>
      </div>
    </li>
  );
}

function compareScheduledTasks(left: ScheduledTask, right: ScheduledTask) {
  if (left.enabled !== right.enabled) {
    return left.enabled ? -1 : 1;
  }

  if (left.nextRunAt === null && right.nextRunAt !== null) {
    return 1;
  }
  if (left.nextRunAt !== null && right.nextRunAt === null) {
    return -1;
  }
  if (left.nextRunAt !== null && right.nextRunAt !== null) {
    return left.nextRunAt - right.nextRunAt;
  }

  return left.name.localeCompare(right.name);
}

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Plus, Pencil, Trash2, Clock, Zap, FolderOpen } from 'lucide-react';
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { ScheduledTaskModal } from './components/ScheduledTaskModal';
import { describeCron, getDisplayCron } from './components/ScheduleBuilder';
import { useScheduledTasks } from './hooks/useScheduledTasks';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useAssistantContext } from '@/context/AssistantContext';
import { formatDateTime } from '@/lib/date-utils';

const logger = getLogger('ScheduledTasksPage');

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

  const handleCreate = async (data: {
    name: string;
    cronExpression: string;
    scheduleTimezone: 'local';
    assistantId: string;
    message: string;
    yoloMode: boolean;
    workspaceOverride: string | null;
  }) => {
    await createTask(data);
  };

  const handleUpdate = async (data: {
    name: string;
    cronExpression: string;
    scheduleTimezone: 'local';
    assistantId: string;
    message: string;
    yoloMode: boolean;
    workspaceOverride: string | null;
  }) => {
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
    return formatDateTime(d);
  };

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

      {tasks.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-16 border border-dashed rounded-lg text-muted-foreground">
          <Clock className="w-8 h-8 opacity-40" />
          <p className="text-sm">{t('scheduledTasks.noTasks')}</p>
          <Button variant="outline" size="sm" onClick={openCreate}>
            {t('scheduledTasks.createFirst')}
          </Button>
        </div>
      ) : (
        <ul className="flex flex-col gap-3">
          {tasks.map((task) => (
            <li
              key={task.id}
              className="flex items-start gap-4 rounded-lg border border-border p-4 bg-card"
            >
              <Switch
                checked={task.enabled}
                onCheckedChange={() => void handleToggle(task)}
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
                      onClick={() => openEdit(task)}
                      aria-label={t('scheduledTasks.editTaskAria', {
                        name: task.name,
                      })}
                    >
                      <Pencil className="w-4 h-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t('scheduledTasks.editTask')}
                  </TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-destructive hover:text-destructive"
                      onClick={() => void handleDelete(task.id)}
                      aria-label={t('scheduledTasks.deleteTaskAria', {
                        name: task.name,
                      })}
                      disabled={
                        deletingIds.has(task.id) || togglingIds.has(task.id)
                      }
                    >
                      {deletingIds.has(task.id) ? (
                        <Clock className="w-4 h-4 animate-spin" />
                      ) : (
                        <Trash2 className="w-4 h-4" />
                      )}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t('scheduledTasks.deleteTask')}
                  </TooltipContent>
                </Tooltip>
              </div>
            </li>
          ))}
        </ul>
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

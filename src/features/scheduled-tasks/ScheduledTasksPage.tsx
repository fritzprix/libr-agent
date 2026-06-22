import { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { Plus } from 'lucide-react';
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { ScheduledTaskModal } from './components/ScheduledTaskModal';
import { ScheduledTasksContent } from './components/ScheduledTaskRow';
import { useScheduledTasks } from './hooks/useScheduledTasks';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useAssistantContext } from '@/context/AssistantContext';
import { getDateTimeFormatter } from '@/lib/date-utils';
import { compareScheduledTasks } from './scheduled-task-utils';
import type { ExecutionMode } from '@/context/agent-session/types';

const logger = getLogger('ScheduledTasksPage');

interface ScheduledTaskFormData {
  name: string;
  cronExpression: string;
  scheduleTimezone: 'local';
  assistantId: string;
  message: string;
  executionMode: ExecutionMode;
  workspaceOverride: string | null;
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

  const handleToggle = useCallback(
    async (task: ScheduledTask) => {
      try {
        await toggleTask(task);
      } catch (error) {
        logger.error('Failed to toggle scheduled task', error);
        toast.error(
          t('scheduledTasks.toggleFailed', 'Failed to update scheduled task'),
        );
      }
    },
    [toggleTask, t],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteTask(id);
      } catch (error) {
        logger.error('Failed to delete scheduled task', error);
        toast.error(
          t('scheduledTasks.deleteFailed', 'Failed to delete scheduled task'),
        );
      }
    },
    [deleteTask, t],
  );

  const openCreate = useCallback(() => {
    setEditingTask(null);
    setModalOpen(true);
  }, []);

  const openEdit = useCallback((task: ScheduledTask) => {
    setEditingTask(task);
    setModalOpen(true);
  }, []);

  const formatNextRun = useCallback(
    (ms: number | null): string => {
      if (!ms) return t('scheduledTasks.nextRunNone');
      const d = new Date(ms);
      return getDateTimeFormatter().format(d);
    },
    [t],
  );

  const sortedTasks = useMemo(
    () => [...tasks].sort(compareScheduledTasks),
    [tasks],
  );

  const enabledTaskCount = useMemo(() => {
    let count = 0;
    for (const task of tasks) {
      if (task.enabled) {
        count++;
      }
    }
    return count;
  }, [tasks]);

  if (loading) {
    return (
      <div className="flex h-full flex-col bg-background p-6">
        <div
          className="mx-auto flex h-full w-full max-w-3xl flex-col"
          role="status"
          aria-busy="true"
        >
          <span className="sr-only">{t('common.loading')}</span>
          <div
            className="mb-6 flex items-center justify-between"
            aria-hidden="true"
          >
            <div>
              <Skeleton className="mb-1 h-7 w-48" />
              <Skeleton className="h-4 w-64" />
            </div>
            <Skeleton className="h-9 w-24" />
          </div>
          <div
            className="min-h-0 flex-1 overflow-y-auto pr-2 pb-4"
            aria-hidden="true"
          >
            <Skeleton className="h-24 w-full rounded-lg" />
            <Skeleton className="mt-3 h-24 w-full rounded-lg" />
            <Skeleton className="mt-3 h-24 w-full rounded-lg" />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-background p-6">
      <div className="mx-auto flex h-full w-full max-w-3xl flex-col">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold">
              {t('scheduledTasks.title')}
            </h1>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {t('scheduledTasks.subtitle')}
            </p>
          </div>
          <Button onClick={openCreate} size="sm">
            <Plus className="mr-1 h-4 w-4" />
            {t('scheduledTasks.newTask')}
          </Button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto pr-2 pb-4">
          <ScheduledTasksContent
            enabledTaskCount={enabledTaskCount}
            formatNextRun={formatNextRun}
            onCreate={openCreate}
            onDelete={handleDelete}
            onEdit={openEdit}
            onToggle={handleToggle}
            sortedTasks={sortedTasks}
            tasks={tasks}
            deletingIds={deletingIds}
            togglingIds={togglingIds}
          />
        </div>
      </div>

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

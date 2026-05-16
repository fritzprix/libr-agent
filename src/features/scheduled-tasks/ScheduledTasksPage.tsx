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
import {
  buildScheduledTaskGroups,
  compareScheduledTasks,
  type ScheduledTaskGroupSection,
} from './scheduled-task-utils';

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

  const groupedSections = useMemo<ScheduledTaskGroupSection[]>(() => {
    return buildScheduledTaskGroups(tasks);
  }, [tasks]);

  const personalTasks = useMemo(
    () => tasks.filter((task) => !task.groupName).sort(compareScheduledTasks),
    [tasks],
  );

  const enabledTaskCount = useMemo(() => {
    // ⚡ Bolt: Replaced .reduce() with for-loop to avoid O(N) functional callback overhead
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

      <ScheduledTasksContent
        enabledTaskCount={enabledTaskCount}
        formatNextRun={formatNextRun}
        groupedSections={groupedSections}
        onCreate={openCreate}
        onDelete={handleDelete}
        onEdit={openEdit}
        onToggle={handleToggle}
        personalTasks={personalTasks}
        tasks={tasks}
        deletingIds={deletingIds}
        togglingIds={togglingIds}
      />

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

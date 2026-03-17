import { useState, useCallback, useEffect } from 'react';
import {
  listScheduledTasks,
  createScheduledTask,
  updateScheduledTask,
  toggleScheduledTask,
  deleteScheduledTask,
  type ScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useScheduledTasks');

export function useScheduledTasks() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [loading, setLoading] = useState(true);

  // Use Sets to keep track of tasks that are currently transitioning
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());

  const loadTasks = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listScheduledTasks();
      setTasks(result);
    } catch (e: unknown) {
      logger.error('Failed to load scheduled tasks', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  const createTask = useCallback(
    async (data: {
      name: string;
      cronExpression: string;
      assistantId: string;
      message: string;
      yoloMode: boolean;
    }) => {
      try {
        const task = await createScheduledTask(data);
        setTasks((prev) => [...prev, task]);
        return task;
      } catch (e: unknown) {
        logger.error('Failed to create scheduled task', e);
        throw e;
      }
    },
    [],
  );

  const updateTask = useCallback(
    async (
      id: string,
      data: {
        name: string;
        cronExpression: string;
        assistantId: string;
        message: string;
        yoloMode: boolean;
      },
    ) => {
      try {
        const updated = await updateScheduledTask(id, data);
        setTasks((prev) =>
          prev.map((t) => (t.id === updated.id ? updated : t)),
        );
        return updated;
      } catch (e: unknown) {
        logger.error('Failed to update scheduled task', e);
        throw e;
      }
    },
    [],
  );

  const toggleTask = useCallback(
    async (task: ScheduledTask) => {
      if (togglingIds.has(task.id) || deletingIds.has(task.id)) return;
      setTogglingIds((prev) => new Set(prev).add(task.id));
      try {
        const updated = await toggleScheduledTask(task.id, !task.enabled);
        setTasks((prev) =>
          prev.map((t) => (t.id === updated.id ? updated : t)),
        );
        return updated;
      } catch (e: unknown) {
        logger.error('Failed to toggle task', e);
        throw e;
      } finally {
        setTogglingIds((prev) => {
          const next = new Set(prev);
          next.delete(task.id);
          return next;
        });
      }
    },
    [togglingIds, deletingIds],
  );

  const deleteTask = useCallback(
    async (id: string) => {
      if (deletingIds.has(id) || togglingIds.has(id)) return;
      setDeletingIds((prev) => new Set(prev).add(id));
      try {
        await deleteScheduledTask(id);
        setTasks((prev) => prev.filter((t) => t.id !== id));
      } catch (e: unknown) {
        logger.error('Failed to delete task', e);
        throw e;
      } finally {
        setDeletingIds((prev) => {
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
      }
    },
    [deletingIds, togglingIds],
  );

  return {
    tasks,
    loading,
    togglingIds,
    deletingIds,
    loadTasks,
    createTask,
    updateTask,
    toggleTask,
    deleteTask,
  };
}

import { useState, useCallback, useEffect, useRef } from 'react';
import {
  listScheduledTasks,
  createScheduledTask,
  updateScheduledTask,
  toggleScheduledTask,
  deleteScheduledTask,
  type ScheduledTask,
  type CreateScheduledTaskRequest,
  type UpdateScheduledTaskRequest,
} from '@/lib/backend/scheduled-tasks';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useScheduledTasks');

export function useScheduledTasks() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [loading, setLoading] = useState(true);

  // Use Sets to keep track of tasks that are currently transitioning
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const togglingIdsRef = useRef<Set<string>>(new Set());
  const deletingIdsRef = useRef<Set<string>>(new Set());

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

  const createTask = useCallback(async (data: CreateScheduledTaskRequest) => {
    try {
      const task = await createScheduledTask(data);
      setTasks((prev) => [...prev, task]);
      return task;
    } catch (e: unknown) {
      logger.error('Failed to create scheduled task', e);
      throw e;
    }
  }, []);

  const updateTask = useCallback(
    async (id: string, data: UpdateScheduledTaskRequest) => {
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

  const toggleTask = useCallback(async (task: ScheduledTask) => {
    if (
      togglingIdsRef.current.has(task.id) ||
      deletingIdsRef.current.has(task.id)
    ) {
      return;
    }
    togglingIdsRef.current.add(task.id);
    setTogglingIds((prev) => new Set(prev).add(task.id));
    try {
      const updated = await toggleScheduledTask(task.id, !task.enabled);
      setTasks((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
      return updated;
    } catch (e: unknown) {
      logger.error('Failed to toggle task', e);
      throw e;
    } finally {
      togglingIdsRef.current.delete(task.id);
      setTogglingIds((prev) => {
        const next = new Set(prev);
        next.delete(task.id);
        return next;
      });
    }
  }, []);

  const deleteTask = useCallback(async (id: string) => {
    if (deletingIdsRef.current.has(id) || togglingIdsRef.current.has(id)) {
      return;
    }
    deletingIdsRef.current.add(id);
    setDeletingIds((prev) => new Set(prev).add(id));
    try {
      await deleteScheduledTask(id);
      setTasks((prev) => prev.filter((t) => t.id !== id));
    } catch (e: unknown) {
      logger.error('Failed to delete task', e);
      throw e;
    } finally {
      deletingIdsRef.current.delete(id);
      setDeletingIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }, []);

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

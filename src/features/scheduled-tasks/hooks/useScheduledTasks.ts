import { useState, useCallback, useRef } from 'react';
import useSWR from 'swr';
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
  const {
    data: tasks = [],
    isLoading,
    isValidating,
    mutate,
  } = useSWR<ScheduledTask[]>('scheduled-tasks', () => listScheduledTasks(), {
    revalidateOnFocus: false,
    onError: (error: unknown) => {
      logger.error('Failed to load scheduled tasks', error);
    },
  });

  const loading = isLoading || isValidating;

  // Use Sets to keep track of tasks that are currently transitioning
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const togglingIdsRef = useRef<Set<string>>(new Set());
  const deletingIdsRef = useRef<Set<string>>(new Set());

  const createTask = useCallback(
    async (data: CreateScheduledTaskRequest) => {
      try {
        const task = await createScheduledTask(data);
        await mutate((prev = []) => [...prev, task], { revalidate: false });
        return task;
      } catch (e: unknown) {
        logger.error('Failed to create scheduled task', e);
        throw e;
      }
    },
    [mutate],
  );

  const updateTask = useCallback(
    async (id: string, data: UpdateScheduledTaskRequest) => {
      try {
        const updated = await updateScheduledTask(id, data);
        await mutate(
          (prev = []) => prev.map((t) => (t.id === updated.id ? updated : t)),
          { revalidate: false },
        );
        return updated;
      } catch (e: unknown) {
        logger.error('Failed to update scheduled task', e);
        throw e;
      }
    },
    [mutate],
  );

  const toggleTask = useCallback(
    async (task: ScheduledTask) => {
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
        await mutate(
          (prev = []) => prev.map((t) => (t.id === updated.id ? updated : t)),
          { revalidate: false },
        );
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
    },
    [mutate],
  );

  const deleteTask = useCallback(
    async (id: string) => {
      if (deletingIdsRef.current.has(id) || togglingIdsRef.current.has(id)) {
        return;
      }
      deletingIdsRef.current.add(id);
      setDeletingIds((prev) => new Set(prev).add(id));
      try {
        await deleteScheduledTask(id);
        await mutate((prev = []) => prev.filter((t) => t.id !== id), {
          revalidate: false,
        });
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
    },
    [mutate],
  );

  const loadTasks = useCallback(async () => {
    try {
      await mutate();
    } catch (e: unknown) {
      logger.error('Failed to reload scheduled tasks', e);
    }
  }, [mutate]);

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

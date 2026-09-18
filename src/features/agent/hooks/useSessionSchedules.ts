import { useCallback, useRef, useState } from 'react';
import useSWR from 'swr';
import {
  cancelSessionScheduledTask,
  listSessionScheduledTasks,
  toggleSessionScheduledTask,
  type SessionScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useSessionSchedules');

export function useSessionSchedules(
  sessionId: string | undefined,
  enabled: boolean,
) {
  const {
    data: tasks = [],
    isLoading,
    mutate,
  } = useSWR<SessionScheduledTask[]>(
    enabled && sessionId ? ['session-scheduled-tasks', sessionId] : null,
    () => listSessionScheduledTasks(sessionId!),
    {
      refreshInterval: 15_000,
      revalidateOnFocus: true,
      onError: (error: unknown) => {
        logger.error('Failed to load session schedules', error);
      },
    },
  );

  const [cancellingIds, setCancellingIds] = useState<Set<string>>(new Set());
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  const cancellingIdsRef = useRef<Set<string>>(new Set());
  const togglingIdsRef = useRef<Set<string>>(new Set());

  const cancelTask = useCallback(
    async (taskId: string) => {
      if (
        !sessionId ||
        cancellingIdsRef.current.has(taskId) ||
        togglingIdsRef.current.has(taskId)
      ) {
        return;
      }

      cancellingIdsRef.current.add(taskId);
      setCancellingIds((prev) => new Set(prev).add(taskId));
      try {
        await cancelSessionScheduledTask(sessionId, taskId);
        await mutate((prev = []) => prev.filter((task) => task.id !== taskId), {
          revalidate: false,
        });
      } catch (error: unknown) {
        logger.error('Failed to cancel session schedule', error);
        throw error;
      } finally {
        cancellingIdsRef.current.delete(taskId);
        setCancellingIds((prev) => {
          const next = new Set(prev);
          next.delete(taskId);
          return next;
        });
      }
    },
    [mutate, sessionId],
  );

  const toggleTask = useCallback(
    async (task: SessionScheduledTask) => {
      if (
        !sessionId ||
        togglingIdsRef.current.has(task.id) ||
        cancellingIdsRef.current.has(task.id)
      ) {
        return;
      }

      togglingIdsRef.current.add(task.id);
      setTogglingIds((prev) => new Set(prev).add(task.id));
      try {
        const updated = await toggleSessionScheduledTask(
          sessionId,
          task.id,
          !task.enabled,
        );
        await mutate(
          (prev = []) =>
            prev.map((item) => (item.id === updated.id ? updated : item)),
          { revalidate: false },
        );
        return updated;
      } catch (error: unknown) {
        logger.error('Failed to toggle session schedule', error);
        throw error;
      } finally {
        togglingIdsRef.current.delete(task.id);
        setTogglingIds((prev) => {
          const next = new Set(prev);
          next.delete(task.id);
          return next;
        });
      }
    },
    [mutate, sessionId],
  );

  return {
    tasks,
    loading: isLoading,
    cancellingIds,
    togglingIds,
    cancelTask,
    toggleTask,
    refresh: mutate,
  };
}

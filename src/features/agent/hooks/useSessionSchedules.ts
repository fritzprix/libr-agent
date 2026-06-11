import { useCallback, useState } from 'react';
import useSWR from 'swr';
import {
  cancelSessionScheduledTask,
  listSessionScheduledTasks,
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

  const cancelTask = useCallback(
    async (taskId: string) => {
      if (!sessionId || cancellingIds.has(taskId)) {
        return;
      }

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
        setCancellingIds((prev) => {
          const next = new Set(prev);
          next.delete(taskId);
          return next;
        });
      }
    },
    [cancellingIds, mutate, sessionId],
  );

  return {
    tasks,
    loading: isLoading,
    cancellingIds,
    cancelTask,
    refresh: mutate,
  };
}

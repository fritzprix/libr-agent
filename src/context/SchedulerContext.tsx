import React, {
  createContext,
  useContext,
  useCallback,
  useEffect,
  ReactNode,
  useMemo,
} from 'react';
import { useAsyncFn, useQueue } from 'react-use';

/**
 * Task type: an async function to be scheduled and executed by the scheduler.
 */
type Task = () => Promise<void>;

/**
 * SchedulerContextType defines the scheduler API exposed to consumers.
 * - schedule: Schedules a task to run when idle.
 * - idle: True if no task is running and the queue is empty.
 */
interface SchedulerContextType {
  schedule: (task: Task) => void;
  idle: boolean;
}

const SchedulerContext = createContext<SchedulerContextType | undefined>(
  undefined,
);

export const SchedulerProvider: React.FC<{ children: ReactNode }> = ({
  children,
}) => {
  // Internal task queue
  const { remove, size, add, first } = useQueue<Task>();
  // Async runner with error handling
  const [{ loading }, run] = useAsyncFn(async (task: Task) => {
    try {
      await task();
    } catch (e) {
      // Log or handle task errors to avoid unhandled promise rejections
      console.error('Scheduled task failed:', e);
    }
  }, []);

  // Run next task when idle and queue is not empty
  useEffect(() => {
    if (first && !loading) {
      run(first).finally(() => {
        remove(); // Only remove after completion
      });
    }
  }, [loading, first, run, remove]);

  // Idle: true if not loading and queue is empty
  const idle = useMemo(() => !loading && size === 0, [loading, size]);

  /**
   * Schedules a new task to be executed when the scheduler is idle.
   */
  const schedule = useCallback(
    (task: Task) => {
      add(task);
    },
    [add],
  );

  const value = { schedule, idle };

  return (
    <SchedulerContext.Provider value={value}>
      {children}
    </SchedulerContext.Provider>
  );
};

/**
 * useScheduler hook for accessing the scheduler context.
 * Throws if used outside a SchedulerProvider.
 */
export function useScheduler() {
  const context = useContext(SchedulerContext);
  if (context === undefined) {
    throw new Error('useScheduler must be used within a SchedulerProvider');
  }
  return context;
}

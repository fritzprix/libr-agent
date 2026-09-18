import { act, renderHook, waitFor } from '@testing-library/react';
import { SWRConfig } from 'swr';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSessionSchedules } from '../useSessionSchedules';
import type { SessionScheduledTask } from '@/lib/backend/scheduled-tasks';
import * as scheduledTasksBackend from '@/lib/backend/scheduled-tasks';

vi.mock('@/lib/backend/scheduled-tasks', () => ({
  listSessionScheduledTasks: vi.fn(),
  toggleSessionScheduledTask: vi.fn(),
  cancelSessionScheduledTask: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const wrapper = ({ children }: { children: ReactNode }) => (
  <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
    {children}
  </SWRConfig>
);

const activeTask: SessionScheduledTask = {
  id: 'task-1',
  name: 'Follow up',
  message: 'Check the thread',
  sessionId: 'session-1',
  isOneShot: true,
  enabled: true,
  nextRunAt: Date.now() + 60_000,
};

describe('useSessionSchedules', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads session callbacks when enabled', async () => {
    vi.mocked(scheduledTasksBackend.listSessionScheduledTasks).mockResolvedValue(
      [activeTask],
    );

    const { result } = renderHook(
      () => useSessionSchedules('session-1', true),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.tasks).toEqual([activeTask]);
    });
    expect(
      scheduledTasksBackend.listSessionScheduledTasks,
    ).toHaveBeenCalledWith('session-1');
  });

  it('does not fetch when disabled', () => {
    renderHook(() => useSessionSchedules('session-1', false), { wrapper });
    expect(
      scheduledTasksBackend.listSessionScheduledTasks,
    ).not.toHaveBeenCalled();
  });

  it('toggles a callback and updates local state', async () => {
    const pausedTask = { ...activeTask, enabled: false };
    vi.mocked(scheduledTasksBackend.listSessionScheduledTasks).mockResolvedValue(
      [activeTask],
    );
    vi.mocked(
      scheduledTasksBackend.toggleSessionScheduledTask,
    ).mockResolvedValue(pausedTask);

    const { result } = renderHook(
      () => useSessionSchedules('session-1', true),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });

    await act(async () => {
      await result.current.toggleTask(activeTask);
    });

    expect(
      scheduledTasksBackend.toggleSessionScheduledTask,
    ).toHaveBeenCalledWith('session-1', 'task-1', false);
    expect(result.current.tasks[0]).toEqual(pausedTask);
  });

  it('cancels a callback and removes it from local state', async () => {
    vi.mocked(scheduledTasksBackend.listSessionScheduledTasks).mockResolvedValue(
      [activeTask],
    );
    vi.mocked(
      scheduledTasksBackend.cancelSessionScheduledTask,
    ).mockResolvedValue(undefined);

    const { result } = renderHook(
      () => useSessionSchedules('session-1', true),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.tasks).toHaveLength(1);
    });

    await act(async () => {
      await result.current.cancelTask('task-1');
    });

    expect(
      scheduledTasksBackend.cancelSessionScheduledTask,
    ).toHaveBeenCalledWith('session-1', 'task-1');
    expect(result.current.tasks).toEqual([]);
  });
});

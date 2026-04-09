import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  createScheduledTask,
  listScheduledTasks,
  getScheduledTask,
  updateScheduledTask,
  toggleScheduledTask,
  deleteScheduledTask,
} from './scheduled-tasks';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

// Mock logger to avoid test output noise
vi.mock('@/lib/logger', () => ({
  getLogger: vi.fn(() => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  })),
}));

describe('scheduled-tasks backend wrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const mockTask = {
    id: 'task-1',
    name: 'Test Task',
    cronExpression: '* * * * *',
    scheduleTimezone: 'local',
    assistantId: 'assistant-1',
    groupId: null,
    groupName: null,
    message: 'Hello',
    yoloMode: false,
    createdBySessionId: null,
    sessionId: null,
    workspaceOverride: null,
    enabled: true,
    lastRunAt: null,
    nextRunAt: null,
    createdAt: 1000,
    updatedAt: 1000,
  };

  it('createScheduledTask calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTask);

    const request = {
      name: 'Test Task',
      cronExpression: '* * * * *',
      assistantId: 'assistant-1',
      message: 'Hello',
      yoloMode: false,
      workspaceOverride: '/tmp/project',
    };

    const result = await createScheduledTask(request);

    expect(safeInvoke).toHaveBeenCalledWith('create_scheduled_task', {
      request,
    });
    expect(result).toEqual(mockTask);
  });

  it('listScheduledTasks calls safeInvoke with correct arguments (with assistantId)', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([mockTask]);

    const result = await listScheduledTasks('assistant-1');

    expect(safeInvoke).toHaveBeenCalledWith('list_scheduled_tasks', {
      assistantId: 'assistant-1',
    });
    expect(result).toEqual([mockTask]);
  });

  it('listScheduledTasks calls safeInvoke with correct arguments (without assistantId)', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([mockTask]);

    const result = await listScheduledTasks();

    expect(safeInvoke).toHaveBeenCalledWith('list_scheduled_tasks', {
      assistantId: undefined,
    });
    expect(result).toEqual([mockTask]);
  });

  it('getScheduledTask calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTask);

    const result = await getScheduledTask('task-1');

    expect(safeInvoke).toHaveBeenCalledWith('get_scheduled_task', {
      id: 'task-1',
    });
    expect(result).toEqual(mockTask);
  });

  it('updateScheduledTask calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTask);

    const request = { name: 'Updated Task', workspaceOverride: '/tmp/updated' };
    const result = await updateScheduledTask('task-1', request);

    expect(safeInvoke).toHaveBeenCalledWith('update_scheduled_task', {
      id: 'task-1',
      request,
    });
    expect(result).toEqual(mockTask);
  });

  it('toggleScheduledTask calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTask);

    const result = await toggleScheduledTask('task-1', false);

    expect(safeInvoke).toHaveBeenCalledWith('toggle_scheduled_task', {
      id: 'task-1',
      enabled: false,
    });
    expect(result).toEqual(mockTask);
  });

  it('deleteScheduledTask calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await deleteScheduledTask('task-1');

    expect(safeInvoke).toHaveBeenCalledWith('delete_scheduled_task', {
      id: 'task-1',
    });
  });
});

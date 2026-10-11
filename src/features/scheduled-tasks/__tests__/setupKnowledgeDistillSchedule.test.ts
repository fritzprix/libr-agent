import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setupKnowledgeDistillSchedule } from '../setupKnowledgeDistillSchedule';

const listAssistants = vi.fn();
const listScheduledTasks = vi.fn();
const createScheduledTask = vi.fn();
const updateScheduledTask = vi.fn();

vi.mock('@/lib/backend/assistants', () => ({
  listAssistants: (...args: unknown[]) => listAssistants(...args),
}));

vi.mock('@/lib/backend/scheduled-tasks', () => ({
  listScheduledTasks: (...args: unknown[]) => listScheduledTasks(...args),
  createScheduledTask: (...args: unknown[]) => createScheduledTask(...args),
  updateScheduledTask: (...args: unknown[]) => updateScheduledTask(...args),
}));

const t = (key: string, options?: { defaultValue?: string }) =>
  options?.defaultValue ?? key;

describe('setupKnowledgeDistillSchedule', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('creates a new nightly distill task for the preferred assistant', async () => {
    listAssistants.mockResolvedValue([
      {
        id: 'ast-1',
        name: 'Libr Assistant',
        systemPrompt: '',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
        allowedBuiltInServiceAliases: ['knowledge', 'history'],
      },
    ]);
    listScheduledTasks.mockResolvedValue([]);
    createScheduledTask.mockResolvedValue({
      id: 'task-1',
      name: 'Daily durable knowledge distillation',
    });

    const result = await setupKnowledgeDistillSchedule({ t });

    expect(result.created).toBe(true);
    expect(result.assistantId).toBe('ast-1');
    expect(createScheduledTask).toHaveBeenCalledWith(
      expect.objectContaining({
        assistantId: 'ast-1',
        cronExpression: '0 21 * * *',
        executionMode: 'yolo',
        resetPlanningState: true,
        scheduleTimezone: 'local',
      }),
    );
    expect(updateScheduledTask).not.toHaveBeenCalled();
  });

  it('updates an existing task with the same name', async () => {
    listAssistants.mockResolvedValue([
      {
        id: 'ast-1',
        name: 'Libr Assistant',
        systemPrompt: '',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
    ]);
    listScheduledTasks.mockResolvedValue([
      {
        id: 'task-existing',
        name: 'Daily durable knowledge distillation',
      },
    ]);
    updateScheduledTask.mockResolvedValue({
      id: 'task-existing',
      name: 'Daily durable knowledge distillation',
    });

    const result = await setupKnowledgeDistillSchedule({ t });

    expect(result.created).toBe(false);
    expect(updateScheduledTask).toHaveBeenCalledWith(
      'task-existing',
      expect.objectContaining({
        assistantId: 'ast-1',
        enabled: true,
      }),
    );
    expect(createScheduledTask).not.toHaveBeenCalled();
  });

  it('throws when no assistant exists', async () => {
    listAssistants.mockResolvedValue([]);
    listScheduledTasks.mockResolvedValue([]);

    await expect(setupKnowledgeDistillSchedule({ t })).rejects.toThrow(
      /Create an assistant first/,
    );
  });
});

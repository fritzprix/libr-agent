
import { expect, test, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ScheduledTasksPage } from '@/features/scheduled-tasks/ScheduledTasksPage';

vi.mock('@/features/scheduled-tasks/hooks/useScheduledTasks', () => ({
  useScheduledTasks: () => ({
    tasks: [
      {
        id: 'task-1',
        name: 'Test Task 1',
        cronExpression: '* * * * *',
        scheduleTimezone: 'local',
        assistantId: 'ast-1',
        message: 'Hello World',
      executionMode: 'normal' as const,
        createdBySessionId: null,
        sessionId: null,
        taskCategory: 'GLOBAL',
        workspaceOverride: '/tmp/scheduled-task-workspace',
        enabled: true,
        lastRunAt: null,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        nextRunAt: Date.now() + 60000,
      },
      {
        id: 'task-2',
        name: 'Solo Task',
        cronExpression: '0 9 * * *',
        scheduleTimezone: 'local',
        assistantId: 'ast-1',
        message: 'Review metrics',
      executionMode: 'normal' as const,
        createdBySessionId: null,
        sessionId: null,
        taskCategory: 'GLOBAL',
        workspaceOverride: null,
        enabled: false,
        lastRunAt: null,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        nextRunAt: Date.now() + 120000,
      },
    ],
    loading: false,
    togglingIds: new Set(),
    deletingIds: new Set(),
    createTask: vi.fn(),
    updateTask: vi.fn(),
    toggleTask: vi.fn(),
    deleteTask: vi.fn(),
  }),
}));

vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: () => ({
    assistants: [{ id: 'ast-1', name: 'Test Assistant' }],
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { name?: string }) =>
      opts?.name ? `${key} ${opts.name}` : key,
  }),
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/components/ui/tooltip', () => {
  return {
    TooltipProvider: ({ children }: { children: ReactNode }) => (
      <div data-testid="tooltip-provider">{children}</div>
    ),
    Tooltip: ({ children }: { children: ReactNode }) => (
      <div data-testid="tooltip">{children}</div>
    ),
    TooltipTrigger: ({
      children,
    }: {
      children: ReactNode;
      asChild?: boolean;
    }) => <div data-testid="tooltip-trigger">{children}</div>,
    TooltipContent: ({ children }: { children: ReactNode }) => (
      <div data-testid="tooltip-content">{children}</div>
    ),
  };
});

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

test('ScheduledTasksPage renders scheduled tasks in a flat list', async () => {
  render(<ScheduledTasksPage />);

  await waitFor(() => {
    expect(screen.getByText('Test Task 1')).toBeInTheDocument();
  });

  const editButton = screen.getByRole('button', {
    name: /scheduledTasks.editTaskAria Test Task 1/i,
  });
  const deleteButton = screen.getByRole('button', {
    name: /scheduledTasks.deleteTaskAria Test Task 1/i,
  });

  expect(editButton).toBeInTheDocument();
  expect(deleteButton).toBeInTheDocument();
  expect(screen.getAllByTestId('tooltip').length).toBeGreaterThanOrEqual(2);
  expect(screen.getAllByText('scheduledTasks.editTask')).toHaveLength(2);
  expect(screen.getAllByText('scheduledTasks.deleteTask')).toHaveLength(2);
  expect(screen.getByText('/tmp/scheduled-task-workspace')).toBeInTheDocument();
  expect(screen.getByText('Solo Task')).toBeInTheDocument();
});

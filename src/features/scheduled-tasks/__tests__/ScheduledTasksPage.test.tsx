
import { expect, test, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ScheduledTasksPage } from '@/features/scheduled-tasks/ScheduledTasksPage';

// Mock the hook we created
vi.mock('@/features/scheduled-tasks/hooks/useScheduledTasks', () => ({
  useScheduledTasks: () => ({
    tasks: [
      {
        id: 'task-1',
        name: 'Test Task 1',
        cronExpression: '* * * * *',
        scheduleTimezone: 'local',
        assistantId: 'ast-1',
        groupId: 'group-1',
        groupName: 'Research Team',
        message: 'Hello World',
        yoloMode: false,
        createdBySessionId: null,
        sessionId: null,
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
        groupId: null,
        groupName: null,
        message: 'Review metrics',
        yoloMode: false,
        createdBySessionId: null,
        sessionId: null,
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

// Mock the assistant context
vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: () => ({
    assistants: [
      { id: 'ast-1', name: 'Test Assistant' },
    ],
  }),
}));

// Mock translation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { name?: string }) =>
      opts?.name ? `${key} ${opts.name}` : key,
  }),
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

// No longer need to mock the backend directly since useScheduledTasks is mocked

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

// Mock Tooltip components
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

// Mock matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // Deprecated
    removeListener: vi.fn(), // Deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

test('ScheduledTasksPage renders grouped and standalone scheduled task sections', async () => {
  render(<ScheduledTasksPage />);

  // Wait for tasks to load
  await waitFor(() => {
    expect(screen.getByText('Test Task 1')).toBeInTheDocument();
  });

  // Find edit and delete buttons
  const editButton = screen.getByRole('button', {
    name: /scheduledTasks.editTaskAria Test Task 1/i,
  });
  const deleteButton = screen.getByRole('button', {
    name: /scheduledTasks.deleteTaskAria Test Task 1/i,
  });

  expect(editButton).toBeInTheDocument();
  expect(deleteButton).toBeInTheDocument();

  // Verify Tooltips exist
  const tooltips = screen.getAllByTestId('tooltip');
  expect(tooltips.length).toBeGreaterThanOrEqual(2);

  // Verify Tooltip content matches short labels
  expect(screen.getAllByText('scheduledTasks.editTask')).toHaveLength(2);
  expect(screen.getAllByText('scheduledTasks.deleteTask')).toHaveLength(2);
  expect(screen.getByText('/tmp/scheduled-task-workspace')).toBeInTheDocument();
  expect(
    screen.getByText('scheduledTasks.groupBadge Research Team'),
  ).toBeInTheDocument();
  expect(screen.getByText('scheduledTasks.groupsTitle')).toBeInTheDocument();
  expect(screen.getByText('scheduledTasks.personalTitle')).toBeInTheDocument();
  expect(screen.getByText('Solo Task')).toBeInTheDocument();

  const groupedSection = screen
    .getByText('scheduledTasks.groupsTitle')
    .closest('section');
  const standaloneSection = screen
    .getByText('scheduledTasks.personalTitle')
    .closest('section');

  expect(groupedSection).not.toBeNull();
  expect(standaloneSection).not.toBeNull();

  expect(within(groupedSection as HTMLElement).getByText('Test Task 1')).toBeInTheDocument();
  expect(
    within(groupedSection as HTMLElement).queryByText('Solo Task'),
  ).not.toBeInTheDocument();
  expect(within(standaloneSection as HTMLElement).getByText('Solo Task')).toBeInTheDocument();
  expect(
    within(standaloneSection as HTMLElement).queryByText('Test Task 1'),
  ).not.toBeInTheDocument();
});

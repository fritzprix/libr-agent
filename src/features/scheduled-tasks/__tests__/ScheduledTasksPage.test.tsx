
import { beforeEach, expect, test, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { ScheduledTasksPage } from '@/features/scheduled-tasks/ScheduledTasksPage';

const deleteTask = vi.fn().mockResolvedValue(undefined);

const initialTasks = [
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
];

let mockTasks = [...initialTasks];

vi.mock('@/features/scheduled-tasks/hooks/useScheduledTasks', () => ({
  useScheduledTasks: () => ({
    tasks: mockTasks,
    loading: false,
    togglingIds: new Set(),
    deletingIds: new Set(),
    createTask: vi.fn(),
    updateTask: vi.fn(),
    toggleTask: vi.fn(),
    deleteTask,
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      serviceConfigs: { openai: { apiKey: 'test' } },
      customProviders: [],
    },
    update: vi.fn(),
    loading: false,
  }),
}));

vi.mock('@/features/scheduled-tasks/components/ScheduledTaskModal', () => ({
  ScheduledTaskModal: ({
    open,
    task,
    initialTemplate,
    onClose,
  }: {
    open: boolean;
    task?: { name: string } | null;
    initialTemplate?: { id: string; name: string } | null;
    onClose: () => void;
  }) =>
    open ? (
      <div data-testid="scheduled-task-modal">
        <span data-testid="modal-task-name">{task?.name ?? ''}</span>
        <span data-testid="modal-template-id">{initialTemplate?.id ?? ''}</span>
        <span data-testid="modal-template-name">
          {initialTemplate?.name ?? ''}
        </span>
        <button onClick={onClose} data-testid="modal-close-btn">
          Close Modal
        </button>
      </div>
    ) : null,
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
  render(
    <MemoryRouter>
      <ScheduledTasksPage />
    </MemoryRouter>,
  );

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

test('ScheduledTasksPage confirms before deleting a task', async () => {
  deleteTask.mockClear();
  render(
    <MemoryRouter>
      <ScheduledTasksPage />
    </MemoryRouter>,
  );

  await waitFor(() => {
    expect(screen.getByText('Test Task 1')).toBeInTheDocument();
  });

  fireEvent.click(
    screen.getByRole('button', {
      name: /scheduledTasks.deleteTaskAria Test Task 1/i,
    }),
  );

  expect(
    screen.getByText('scheduledTasks.deleteConfirm.title'),
  ).toBeInTheDocument();
  expect(deleteTask).not.toHaveBeenCalled();

  fireEvent.click(
    screen.getByRole('button', {
      name: 'scheduledTasks.deleteConfirm.confirm',
    }),
  );

  await waitFor(() => {
    expect(deleteTask).toHaveBeenCalledWith('task-1');
  });
});

beforeEach(() => {
  mockTasks = [...initialTasks];
});

test('ScheduledTasksPage renders starter templates when there are no tasks', async () => {
  mockTasks = [];
  render(
    <MemoryRouter>
      <ScheduledTasksPage />
    </MemoryRouter>,
  );

  // Heading / title
  expect(
    screen.getByText('scheduledTasks.starterTemplates.title'),
  ).toBeInTheDocument();

  // Template titles
  expect(
    screen.getByText('scheduledTasks.starterTemplates.pcAuditTitle'),
  ).toBeInTheDocument();
  expect(
    screen.getByText('scheduledTasks.starterTemplates.webSummaryTitle'),
  ).toBeInTheDocument();

  // Template descriptions
  expect(
    screen.getByText('scheduledTasks.starterTemplates.pcAuditDesc'),
  ).toBeInTheDocument();
  expect(
    screen.getByText('scheduledTasks.starterTemplates.webSummaryDesc'),
  ).toBeInTheDocument();

  // Template buttons
  const useTemplateButtons = screen.getAllByRole('button', {
    name: 'scheduledTasks.starterTemplates.useTemplate',
  });
  expect(useTemplateButtons).toHaveLength(2);

  // Click first template ("pc-health-audit")
  fireEvent.click(useTemplateButtons[0]);

  expect(screen.getByTestId('scheduled-task-modal')).toBeInTheDocument();
  expect(screen.getByTestId('modal-template-id')).toHaveTextContent(
    'pc-health-audit',
  );

  // Close modal resets template
  fireEvent.click(screen.getByTestId('modal-close-btn'));
  expect(screen.queryByTestId('scheduled-task-modal')).not.toBeInTheDocument();

  // Click blank task button
  const createBlankButtons = screen.getAllByRole('button', {
    name: 'scheduledTasks.createBlank',
  });
  expect(createBlankButtons.length).toBeGreaterThanOrEqual(1);
  fireEvent.click(createBlankButtons[0]);

  expect(screen.getByTestId('scheduled-task-modal')).toBeInTheDocument();
  expect(screen.getByTestId('modal-template-id')).toHaveTextContent('');
});

test('ScheduledTasksPage renders open walkthrough button and opens dialog when clicked', async () => {
  mockTasks = [];
  render(
    <MemoryRouter>
      <ScheduledTasksPage />
    </MemoryRouter>,
  );

  const walkthroughButton = screen.getByRole('button', {
    name: 'scheduledTasks.starterTemplates.openWalkthrough',
  });
  expect(walkthroughButton).toBeInTheDocument();

  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

  fireEvent.click(walkthroughButton);

  await waitFor(() => {
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(
      screen.getByText(
        /recipes\.morningBriefing\.modalTitle|모닝 테크 & 금융 브리핑 세팅/,
      ),
    ).toBeInTheDocument();
  });
});

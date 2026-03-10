
import { expect, test, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ScheduledTasksPage } from '@/features/scheduled-tasks/ScheduledTasksPage';

// Mock translation
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { name?: string }) => opts?.name ? `${key} ${opts.name}` : key
  }),
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>
}));

// Mock the backend
vi.mock('@/lib/backend/scheduled-tasks', () => ({
  listScheduledTasks: vi.fn().mockResolvedValue([
    {
      id: 'task-1',
      name: 'Test Task 1',
      cronExpression: '* * * * *',
      assistantId: 'ast-1',
      message: 'Hello World',
      enabled: true,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      nextRunAt: Date.now() + 60000
    }
  ]),
  toggleScheduledTask: vi.fn(),
  deleteScheduledTask: vi.fn()
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    debug: vi.fn()
  })
}));

// Mock Tooltip components
vi.mock('@/components/ui/tooltip', () => {
  return {
    TooltipProvider: ({ children }: { children: ReactNode }) => <div data-testid="tooltip-provider">{children}</div>,
    Tooltip: ({ children }: { children: ReactNode }) => <div data-testid="tooltip">{children}</div>,
    TooltipTrigger: ({ children }: { children: ReactNode, asChild?: boolean }) => <div data-testid="tooltip-trigger">{children}</div>,
    TooltipContent: ({ children }: { children: ReactNode }) => <div data-testid="tooltip-content">{children}</div>
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

test('ScheduledTasksPage renders tooltips for edit and delete buttons', async () => {
  render(<ScheduledTasksPage />);

  // Wait for tasks to load
  await waitFor(() => {
    expect(screen.getByText('Test Task 1')).toBeInTheDocument();
  });

  // Find edit and delete buttons
  const editButton = screen.getByRole('button', { name: /scheduledTasks.editTaskAria Test Task 1/i });
  const deleteButton = screen.getByRole('button', { name: /scheduledTasks.deleteTaskAria Test Task 1/i });

  expect(editButton).toBeInTheDocument();
  expect(deleteButton).toBeInTheDocument();

  // Verify Tooltips exist
  const tooltips = screen.getAllByTestId('tooltip');
  expect(tooltips.length).toBeGreaterThanOrEqual(2);

  // Verify Tooltip content matches short labels
  expect(screen.getByText('scheduledTasks.editTask')).toBeInTheDocument();
  expect(screen.getByText('scheduledTasks.deleteTask')).toBeInTheDocument();
});

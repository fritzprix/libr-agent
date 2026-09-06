import React, { type ReactNode } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
  DragAndDropEvent,
  DragAndDropPayload,
} from '@/context/DnDContext';
import * as fileOperations from '@/lib/backend/file-operations';
import type { Assistant } from '@/models/chat';
import { ScheduledTaskModal } from '../ScheduledTaskModal';
import { STARTER_TASK_TEMPLATES } from '../../starter-templates';

const subscribeMock = vi.fn(
  (
    _ref: unknown,
    handler: (event: DragAndDropEvent, payload: DragAndDropPayload) => void,
  ) => {
    latestHandler = handler;
    return vi.fn();
  },
);
let latestHandler:
  | ((event: DragAndDropEvent, payload: DragAndDropPayload) => void)
  | undefined;
let mentionTextareaProps: {
  workspacePath?: string | null;
} | null = null;

vi.mock('@/context/DnDContext', () => ({
  useDnDContext: () => ({
    subscribe: subscribeMock,
  }),
}));

vi.mock('@/lib/backend/file-operations', () => ({
  checkDroppedPathType: vi.fn(),
  registerDroppedFiles: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
  Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('../MentionTextarea', () => ({
  MentionTextarea: ({
    workspacePath,
    value,
    onChange,
    assistantId,
  }: {
    workspacePath?: string | null;
    value?: string;
    onChange?: (val: string) => void;
    assistantId?: string;
  }) => {
    mentionTextareaProps = { workspacePath };
    return (
      <textarea
        data-testid="mention-textarea"
        data-assistant-id={assistantId}
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
      />
    );
  },
}));

vi.mock('../ScheduleBuilder', () => ({
  ScheduleBuilder: ({
    value,
    onChange,
  }: {
    value: string;
    onChange?: (val: string) => void;
  }) => (
    <div data-testid="schedule-builder" data-value={value}>
      <input
        data-testid="schedule-builder-input"
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
      />
    </div>
  ),
  getDisplayCron: (value: string) => value,
}));

vi.mock('@/components/ui/dialog', () => ({
  Dialog: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

vi.mock('@/components/ui/button', () => {
  const MockButton = React.forwardRef<HTMLButtonElement, { children: ReactNode; onClick?: () => void; disabled?: boolean }>(({ children, ...props }, ref) => (
    <button ref={ref} {...props}>{children}</button>
  ));
  MockButton.displayName = 'Button';
  return { Button: MockButton };
});

vi.mock('@/components/ui/input', () => ({
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => (
    <input data-testid={props.id || 'input'} {...props} />
  ),
}));

vi.mock('@/components/ui/label', () => ({
  Label: ({ children, htmlFor }: { children: ReactNode; htmlFor?: string }) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

vi.mock('@/components/ui/select', () => ({
  Select: ({
    children,
    value,
  }: {
    children: ReactNode;
    value?: string;
  }) => (
    <div data-testid="select" data-value={value}>
      {children}
    </div>
  ),
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({
    children,
    value,
  }: {
    children: ReactNode;
    value: string;
  }) => <div data-testid={`select-item-${value}`}>{children}</div>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: () => <div />,
}));

vi.mock('@/components/ui/switch', () => ({
  Switch: ({
    checked,
    onCheckedChange,
    id,
  }: {
    checked?: boolean;
    onCheckedChange?: (c: boolean) => void;
    id?: string;
  }) => (
    <button
      type="button"
      id={id}
      data-testid={id ?? 'switch'}
      role="switch"
      aria-checked={checked}
      onClick={() => onCheckedChange?.(!checked)}
    />
  ),
}));

vi.mock('lucide-react', () => ({
  Zap: () => <span />,
  Shield: () => <span />,
  DatabaseZap: () => <span />,
  Loader2: () => <span />,
  FolderOpen: () => <span />,
  Upload: () => <span />,
  X: () => <span />,
  ChevronDown: () => <span />,
  ListTodo: () => <span />,
}));

vi.mock('@/components/ui/collapsible', () => ({
  Collapsible: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CollapsibleTrigger: ({ children }: { children: ReactNode }) => (
    <button type="button">{children}</button>
  ),
  CollapsibleContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
}));

beforeEach(() => {
  vi.clearAllMocks();
  latestHandler = undefined;
  mentionTextareaProps = null;
});

test('ScheduledTaskModal accepts a dropped directory as workspace override', async () => {
  vi.mocked(fileOperations.registerDroppedFiles).mockResolvedValue();
  vi.mocked(fileOperations.checkDroppedPathType).mockResolvedValue('directory');
  const assistant: Assistant = {
    id: 'assistant-1',
    name: 'Assistant 1',
    systemPrompt: 'Test assistant',
    deletionProtected: false,
    createdAt: new Date('2026-01-01T00:00:00.000Z'),
    updatedAt: new Date('2026-01-01T00:00:00.000Z'),
  };

  render(
    <ScheduledTaskModal
      open
      assistants={[assistant]}
      onClose={vi.fn()}
      onSave={vi.fn()}
    />,
  );

  expect(subscribeMock).toHaveBeenCalled();

  await act(async () => {
    latestHandler?.('drop', { paths: ['/tmp/workspace-folder'] });
  });

  await waitFor(() => {
    expect(screen.getByText('/tmp/workspace-folder')).toBeInTheDocument();
  });

  expect(vi.mocked(fileOperations.registerDroppedFiles)).toHaveBeenCalledWith([
    '/tmp/workspace-folder',
  ]);
  expect(mentionTextareaProps).toEqual({
    workspacePath: '/tmp/workspace-folder',
  });
});

test('ScheduledTaskModal pre-populates form fields when initialTemplate is provided', async () => {
  const template = STARTER_TASK_TEMPLATES[0]; // pc-health-audit (unsafe mode, resetPlanningState: true)
  const onSave = vi.fn().mockResolvedValue(undefined);
  const assistants: Assistant[] = [
    {
      id: 'ast-default',
      name: 'Default Assistant',
      systemPrompt: 'Default',
      deletionProtected: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    },
    {
      id: 'ast-expert',
      name: 'Coding Expert',
      systemPrompt: 'Expert',
      deletionProtected: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    },
  ];

  render(
    <ScheduledTaskModal
      open
      initialTemplate={template}
      assistants={assistants}
      onClose={vi.fn()}
      onSave={onSave}
    />,
  );

  // Name pre-populated
  const nameInput = screen.getByTestId('task-name') as HTMLInputElement;
  expect(nameInput.value).toBe(template.name);

  // Cron pre-populated
  const cronInput = screen.getByTestId(
    'schedule-builder-input',
  ) as HTMLInputElement;
  expect(cronInput.value).toBe(template.cronExpression);

  // Message pre-populated
  const textarea = screen.getByTestId('mention-textarea') as HTMLTextAreaElement;
  expect(textarea.value).toBe(template.message);

  // Assistant matched by preferredAssistantName
  expect(textarea.getAttribute('data-assistant-id')).toBe('ast-expert');

  // Reset planning state switch is checked
  const switchBtn = screen.getByTestId('reset-planning-state');
  expect(switchBtn).toHaveAttribute('aria-checked', 'true');

  // Unsafe notice is displayed
  expect(screen.getByRole('alert')).toBeInTheDocument();
  expect(
    screen.getByText(/무인 실행 중 터미널 명령어|scheduledTasks\.modal\.unsafeNotice/),
  ).toBeInTheDocument();

  // Save calls onSave with template values
  const saveButton = screen.getByRole('button', {
    name: 'scheduledTasks.modal.createTask',
  });
  fireEvent.click(saveButton);

  await waitFor(() => {
    expect(onSave).toHaveBeenCalledWith({
      name: template.name,
      cronExpression: template.cronExpression,
      scheduleTimezone: 'local',
      assistantId: 'ast-expert',
      message: template.message,
      executionMode: 'unsafe',
      workspaceOverride: null,
      resetPlanningState: true,
    });
  });
});

test('ScheduledTaskModal falls back to first assistant if preferredAssistantName is not matched', async () => {
  const template = STARTER_TASK_TEMPLATES[0]; // pc-health-audit
  const onSave = vi.fn().mockResolvedValue(undefined);
  const assistants: Assistant[] = [
    {
      id: 'ast-only',
      name: 'Some Other Assistant',
      systemPrompt: 'Other',
      deletionProtected: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    },
  ];

  render(
    <ScheduledTaskModal
      open
      initialTemplate={template}
      assistants={assistants}
      onClose={vi.fn()}
      onSave={onSave}
    />,
  );

  const saveButton = screen.getByRole('button', {
    name: 'scheduledTasks.modal.createTask',
  });
  fireEvent.click(saveButton);

  await waitFor(() => {
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        assistantId: 'ast-only',
      }),
    );
  });
});

test('ScheduledTaskModal does not display unsafe notice for yolo execution mode', () => {
  const template = STARTER_TASK_TEMPLATES[1]; // web-headline-summary (yolo)
  const assistants: Assistant[] = [
    {
      id: 'ast-1',
      name: 'Libr Assistant',
      systemPrompt: 'Libr',
      deletionProtected: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    },
  ];

  render(
    <ScheduledTaskModal
      open
      initialTemplate={template}
      assistants={assistants}
      onClose={vi.fn()}
      onSave={vi.fn()}
    />,
  );

  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

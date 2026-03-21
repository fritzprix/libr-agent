import { act, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
  DragAndDropEvent,
  DragAndDropPayload,
} from '@/context/DnDContext';
import * as fileOperations from '@/lib/backend/file-operations';
import type { Assistant } from '@/models/chat';
import { ScheduledTaskModal } from '../ScheduledTaskModal';

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
  }: {
    workspacePath?: string | null;
  }) => {
    mentionTextareaProps = { workspacePath };
    return <div data-testid="mention-textarea" />;
  },
}));

vi.mock('../ScheduleBuilder', () => ({
  ScheduleBuilder: () => <div data-testid="schedule-builder" />,
  getDisplayCron: (value: string) => value,
}));

vi.mock('@/components/ui/dialog', () => ({
  Dialog: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

vi.mock('@/components/ui/button', () => ({
  Button: ({ children }: { children: ReactNode }) => <button>{children}</button>,
}));

vi.mock('@/components/ui/input', () => ({
  Input: () => <input />,
}));

vi.mock('@/components/ui/label', () => ({
  Label: ({ children }: { children: ReactNode }) => <label>{children}</label>,
}));

vi.mock('@/components/ui/select', () => ({
  Select: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: () => <div />,
}));

vi.mock('@/components/ui/switch', () => ({
  Switch: () => <button type="button" />,
}));

vi.mock('lucide-react', () => ({
  Zap: () => <span />,
  FolderOpen: () => <span />,
  Upload: () => <span />,
  X: () => <span />,
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

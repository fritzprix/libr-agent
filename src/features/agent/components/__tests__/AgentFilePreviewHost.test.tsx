import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentFilePreviewHost } from '../AgentFilePreviewHost';

const mockCloseFilePreview = vi.fn();
let mockPreviewFile: {
  name: string;
  path: string;
  size?: number;
  sessionId?: string;
} | null = null;

vi.mock('@/context/AgentFilePreviewContext', () => ({
  useAgentFilePreview: () => ({
    previewFile: mockPreviewFile,
    closeFilePreview: mockCloseFilePreview,
  }),
}));

let mockSession: { id: string } | null = { id: 'session-default' };

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: mockSession,
  }),
}));

const mockOpenWorkspaceFileWithDefaultApp = vi.fn();

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openWorkspaceFileWithDefaultApp: mockOpenWorkspaceFileWithDefaultApp,
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (options?.name) return `${key}:${options.name}`;
      return key;
    },
  }),
}));

const mockToastSuccess = vi.fn();
const mockToastError = vi.fn();
vi.mock('sonner', () => ({
  toast: {
    success: (...args: unknown[]) => mockToastSuccess(...args),
    error: (...args: unknown[]) => mockToastError(...args),
  },
}));

vi.mock('../workspace-panel/WorkspaceFilePreviewSheet', () => ({
  WorkspaceFilePreviewSheet: ({
    file,
    sessionId,
    isOpen,
    onClose,
    onOpenInDefaultApp,
  }: {
    file: { name: string; path: string } | null;
    sessionId?: string;
    isOpen: boolean;
    onClose: () => void;
    onOpenInDefaultApp: (path: string) => Promise<void>;
  }) => (
    <div
      data-testid="preview-sheet"
      data-is-open={String(isOpen)}
      data-session-id={sessionId}
    >
      {file && <span data-testid="sheet-file-name">{file.name}</span>}
      <button data-testid="sheet-close-btn" onClick={onClose}>
        Close
      </button>
      <button
        data-testid="sheet-open-default-btn"
        onClick={() => onOpenInDefaultApp(file?.path ?? '')}
      >
        Open App
      </button>
    </div>
  ),
}));

describe('AgentFilePreviewHost', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPreviewFile = null;
    mockSession = { id: 'session-default' };
  });

  it('renders closed sheet when previewFile is null', () => {
    render(<AgentFilePreviewHost />);

    const sheet = screen.getByTestId('preview-sheet');
    expect(sheet).toHaveAttribute('data-is-open', 'false');
    expect(sheet).toHaveAttribute('data-session-id', 'session-default');
    expect(screen.queryByTestId('sheet-file-name')).not.toBeInTheDocument();
  });

  it('renders open sheet with file and explicit previewFile.sessionId', () => {
    mockPreviewFile = {
      name: 'notes.md',
      path: 'docs/notes.md',
      size: 120,
      sessionId: 'custom-session-456',
    };

    render(<AgentFilePreviewHost />);

    const sheet = screen.getByTestId('preview-sheet');
    expect(sheet).toHaveAttribute('data-is-open', 'true');
    expect(sheet).toHaveAttribute('data-session-id', 'custom-session-456');
    expect(screen.getByTestId('sheet-file-name')).toHaveTextContent('notes.md');
  });

  it('delegates onClose to closeFilePreview', () => {
    mockPreviewFile = {
      name: 'test.ts',
      path: 'src/test.ts',
    };

    render(<AgentFilePreviewHost />);

    fireEvent.click(screen.getByTestId('sheet-close-btn'));
    expect(mockCloseFilePreview).toHaveBeenCalledTimes(1);
  });

  it('opens workspace file with default app and shows success toast', async () => {
    mockPreviewFile = {
      name: 'report.pdf',
      path: 'output/report.pdf',
      sessionId: 'session-789',
    };
    mockOpenWorkspaceFileWithDefaultApp.mockResolvedValueOnce(undefined);

    render(<AgentFilePreviewHost />);

    fireEvent.click(screen.getByTestId('sheet-open-default-btn'));

    await waitFor(() => {
      expect(mockOpenWorkspaceFileWithDefaultApp).toHaveBeenCalledWith(
        'output/report.pdf',
        'session-789',
      );
      expect(mockToastSuccess).toHaveBeenCalledWith(
        'agent.workspace.fileOpened',
        expect.objectContaining({
          description: 'agent.workspace.fileOpenedDescription:report.pdf',
        }),
      );
    });
  });

  it('handles error when opening with default app fails', async () => {
    mockPreviewFile = {
      name: 'missing.bin',
      path: 'missing.bin',
    };
    mockOpenWorkspaceFileWithDefaultApp.mockRejectedValueOnce(
      new Error('App not found'),
    );

    render(<AgentFilePreviewHost />);

    fireEvent.click(screen.getByTestId('sheet-open-default-btn'));

    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledWith(
        'agent.workspace.fileOpenError',
        expect.objectContaining({
          description: 'App not found',
        }),
      );
    });
  });
});

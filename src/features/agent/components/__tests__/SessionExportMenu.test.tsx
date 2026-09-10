import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import type { Message } from '@/models/chat';
import { SessionExportMenu } from '../SessionExportMenu';
import { DOWNLOAD_CANCELLED } from '@/lib/notify-file-download';

const mockExportSessionFile = vi.fn();
const mockCopyToClipboard = vi.fn();
const mockNotifyFileDownloadSuccess = vi.fn();
const mockMessagesToMarkdown = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copyToClipboard: mockCopyToClipboard,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipContent: () => null,
}));

vi.mock('@/components/ui/dropdown-menu', () => ({
  DropdownMenu: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
    <div role="menu">{children}</div>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuItem: ({
    children,
    onSelect,
  }: {
    children: React.ReactNode;
    onSelect?: (event: Event) => void;
  }) => (
    <button
      type="button"
      role="menuitem"
      onClick={() =>
        onSelect?.({
          preventDefault() {},
          stopPropagation() {},
        } as Event)
      }
    >
      {children}
    </button>
  ),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

vi.mock('@/lib/backend', () => ({
  exportSessionFile: (...args: unknown[]) => mockExportSessionFile(...args),
}));

vi.mock('@/lib/notify-file-download', async () => {
  const actual = await vi.importActual<
    typeof import('@/lib/notify-file-download')
  >('@/lib/notify-file-download');
  return {
    ...actual,
    notifyFileDownloadSuccess: (...args: unknown[]) =>
      mockNotifyFileDownloadSuccess(...args),
  };
});

vi.mock('@/lib/message-utils', () => ({
  messagesToMarkdown: (...args: unknown[]) => mockMessagesToMarkdown(...args),
}));

const sampleMessage: Message = {
  id: 'msg-1',
  sessionId: 'session-123',
  threadId: 'session-123',
  role: 'user',
  content: [{ type: 'text', text: 'hello' }],
};

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('SessionExportMenu', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockExportSessionFile.mockResolvedValue('/tmp/session.md');
    mockCopyToClipboard.mockResolvedValue(undefined);
    mockMessagesToMarkdown.mockReturnValue({
      content: '## User\n\nhello',
      truncated: false,
    });
  });

  it('exports markdown from the backend snapshot', async () => {
    render(<SessionExportMenu sessionId="session-123" />);

    fireEvent.click(
      screen.getByRole('button', { name: 'agent.sessionExport.menuAria' }),
    );
    fireEvent.click(screen.getByText('agent.sessionExport.markdown'));

    await waitFor(() => {
      expect(mockExportSessionFile).toHaveBeenCalledWith({
        sessionId: 'session-123',
        format: 'markdown',
      });
    });
    expect(mockNotifyFileDownloadSuccess).toHaveBeenCalledWith(
      expect.objectContaining({
        title: 'agent.sessionExport.markdownSuccess',
        filePath: '/tmp/session.md',
      }),
    );
  });

  it('exports an ATIF trajectory from the backend snapshot', async () => {
    mockExportSessionFile.mockResolvedValueOnce('/tmp/session_trajectory.json');
    render(<SessionExportMenu sessionId="session-123" />);

    fireEvent.click(
      screen.getByRole('button', { name: 'agent.sessionExport.menuAria' }),
    );
    fireEvent.click(screen.getByText('agent.sessionExport.atif'));

    await waitFor(() => {
      expect(mockExportSessionFile).toHaveBeenCalledWith({
        sessionId: 'session-123',
        format: 'atif',
      });
    });
    expect(mockNotifyFileDownloadSuccess).toHaveBeenCalledWith(
      expect.objectContaining({
        title: 'agent.sessionExport.atifSuccess',
        filePath: '/tmp/session_trajectory.json',
      }),
    );
  });

  it('does not toast success when the save dialog is cancelled', async () => {
    const { toast } = await import('sonner');
    mockExportSessionFile.mockResolvedValueOnce(DOWNLOAD_CANCELLED);
    render(<SessionExportMenu sessionId="session-123" />);

    fireEvent.click(
      screen.getByRole('button', { name: 'agent.sessionExport.menuAria' }),
    );
    fireEvent.click(screen.getByText('agent.sessionExport.markdown'));

    await waitFor(() => {
      expect(toast.info).toHaveBeenCalledWith(
        'agent.sessionExport.cancelled',
      );
    });
    expect(mockNotifyFileDownloadSuccess).not.toHaveBeenCalled();
  });

  it('copies the in-memory window when clipboard is enabled', async () => {
    const { toast } = await import('sonner');
    render(
      <SessionExportMenu
        sessionId="session-123"
        showClipboardCopy
        messages={[sampleMessage]}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'agent.sessionExport.menuAria' }),
    );
    fireEvent.click(screen.getByText('agent.sessionExport.copyClipboard'));

    await waitFor(() => {
      expect(mockMessagesToMarkdown).toHaveBeenCalledWith([sampleMessage]);
      expect(mockCopyToClipboard).toHaveBeenCalledWith('## User\n\nhello');
    });
    expect(toast.success).toHaveBeenCalledWith('agent.header.copySuccess');
  });

  it('hides clipboard copy on history cards', () => {
    render(<SessionExportMenu sessionId="session-123" />);
    expect(
      screen.queryByText('agent.sessionExport.copyClipboard'),
    ).not.toBeInTheDocument();
  });

  it('toasts an error when backend export fails', async () => {
    const { toast } = await import('sonner');
    mockExportSessionFile.mockRejectedValueOnce(new Error('disk full'));
    render(<SessionExportMenu sessionId="session-123" />);

    fireEvent.click(screen.getByText('agent.sessionExport.markdown'));

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('agent.sessionExport.error');
    });
    expect(mockNotifyFileDownloadSuccess).not.toHaveBeenCalled();
  });
});

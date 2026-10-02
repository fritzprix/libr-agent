import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import { MessageActionBar } from '../MessageActionBar';

const mockCopyToClipboard = vi.fn();
const mockSerialize = vi.fn();
const mockSerializeForDownload = vi.fn();
const mockDownloadTextFile = vi.fn();
const mockDownloadTextPdf = vi.fn();
const mockPrepareMarkdownForPdfExport = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copied: false,
    copyToClipboard: mockCopyToClipboard,
  }),
}));

vi.mock('@/hooks/use-is-dark-mode', () => ({
  useIsDarkMode: () => true,
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <div role="tooltip">{children}</div>
  ),
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

vi.mock('@/features/agent/lib/message-serialization', async () => {
  const actual = await vi.importActual<
    typeof import('@/features/agent/lib/message-serialization')
  >('@/features/agent/lib/message-serialization');
  return {
    ...actual,
    serializeMessageForClipboard: (...args: unknown[]) => mockSerialize(...args),
    serializeMessageForDownload: (...args: unknown[]) =>
      mockSerializeForDownload(...args),
  };
});

vi.mock('@/features/agent/lib/pdf-export-preprocess', () => ({
  prepareMarkdownForPdfExport: (...args: unknown[]) =>
    mockPrepareMarkdownForPdfExport(...args),
}));

vi.mock('@/lib/backend', () => ({
  downloadTextFile: (...args: unknown[]) => mockDownloadTextFile(...args),
  downloadTextPdf: (...args: unknown[]) => mockDownloadTextPdf(...args),
  openPathWithDefaultApp: vi.fn(),
}));

vi.mock('@/lib/notify-file-download', () => ({
  DOWNLOAD_CANCELLED: 'DOWNLOAD_CANCELLED',
  notifyFileDownloadSuccess: vi.fn(),
}));

function createMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'assistant',
    content: [{ type: 'text', text: 'Hello' }],
    ...overrides,
  };
}

describe('MessageActionBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSerialize.mockReturnValue('[{"id":"call-1"}]');
    mockSerializeForDownload.mockReturnValue('## Answer\n\n- point one');
    mockCopyToClipboard.mockResolvedValue(undefined);
    mockDownloadTextFile.mockResolvedValue('/tmp/message.md');
    mockDownloadTextPdf.mockResolvedValue('/tmp/message.pdf');
    mockPrepareMarkdownForPdfExport.mockResolvedValue({
      content: '## Answer\n\n- point one',
      embeddedImages: [],
    });
  });

  it('copies plain text for a normal message (not thinking/tools dump)', async () => {
    render(<MessageActionBar message={createMessage()} />);

    fireEvent.click(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    );

    await waitFor(() => {
      expect(mockCopyToClipboard).toHaveBeenCalledWith('Hello');
    });
    expect(mockSerialize).not.toHaveBeenCalled();
  });

  it('keeps a single primary copy control (no text-only button)', () => {
    render(<MessageActionBar message={createMessage()} />);

    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    ).toBeVisible();
    expect(
      screen.queryByRole('button', {
        name: 'agent.bubble.actionBar.copyTextAria',
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', {
        name: 'agent.bubble.actionBar.copyFullAria',
      }),
    ).not.toBeInTheDocument();
  });

  it('exposes copy, tools, and export actions', () => {
    render(<MessageActionBar message={createMessage()} />);

    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    ).toBeVisible();
    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyToolsAria',
      }),
    ).toBeVisible();
    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.exportAria',
      }),
    ).toBeVisible();
  });

  it('runs PDF preprocess and forwards embeddedImages to downloadTextPdf', async () => {
    const preprocessed = {
      content: '![Mermaid diagram](libragent-pdf-embed:0)\n',
      embeddedImages: [{ dataBase64: 'aaaPNG' }],
    };
    mockPrepareMarkdownForPdfExport.mockResolvedValueOnce(preprocessed);
    mockSerializeForDownload.mockReturnValueOnce(
      '```mermaid\nflowchart TD\nA-->B\n```\n',
    );

    render(<MessageActionBar message={createMessage()} />);

    fireEvent.click(
      screen.getByRole('menuitem', {
        name: 'agent.bubble.actionBar.exportPdf',
      }),
    );

    await waitFor(() => {
      expect(mockPrepareMarkdownForPdfExport).toHaveBeenCalledWith(
        '```mermaid\nflowchart TD\nA-->B\n```\n',
        { isDark: true },
      );
      expect(mockDownloadTextPdf).toHaveBeenCalledWith({
        fileName: 'message.pdf',
        content: preprocessed.content,
        embeddedImages: preprocessed.embeddedImages,
      });
    });
  });

  it('copy prefers reportResult document over wrapper transcript', async () => {
    mockCopyToClipboard.mockResolvedValue(undefined);

    render(
      <MessageActionBar
        message={createMessage({
          role: 'tool',
          content: [{ type: 'text', text: 'Final result reported STOP' }],
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'success',
              title: 'Ship',
              result: 'ok',
              deliverables: [],
            },
          },
        })}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    );

    await waitFor(() => {
      expect(mockCopyToClipboard).toHaveBeenCalled();
    });
    const copied = String(mockCopyToClipboard.mock.calls[0]?.[0] ?? '');
    expect(copied).toContain('# Ship');
    expect(copied).toContain('ok');
    expect(copied).not.toContain('STOP');
  });

  it('copy uses bubble reportResult document, not first narration', async () => {
    mockCopyToClipboard.mockResolvedValue(undefined);

    const displayContent: MCPContent[] = [
      { type: 'text', text: 'FIRST RESPONSE: planning...' },
      {
        type: 'tool_call',
        id: 'call-report',
        name: 'ui__reportResult',
        arguments: '{}',
      },
    ];
    const toolResultsMap = new Map<string, Message>([
      [
        'call-report',
        createMessage({
          id: 'tool-report',
          role: 'tool',
          tool_call_id: 'call-report',
          content: [{ type: 'text', text: 'Final result reported STOP' }],
          metadata: {
            structuredContent: {
              type: 'reportResult',
              status: 'success',
              title: 'Deliverable',
              criteria: 'Must ship',
              proof: 'CI green',
              result: 'REPORT BODY',
              deliverables: [],
            },
          },
        }),
      ],
    ]);

    render(
      <MessageActionBar
        message={createMessage({
          content: [{ type: 'text', text: 'FIRST RESPONSE: planning...' }],
          tool_calls: [
            {
              id: 'call-search',
              type: 'function',
              function: { name: 'workspace__runShell', arguments: '{}' },
            },
          ],
        })}
        displayContent={displayContent}
        toolResultsMap={toolResultsMap}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    );

    await waitFor(() => {
      expect(mockCopyToClipboard).toHaveBeenCalled();
    });
    const copied = String(mockCopyToClipboard.mock.calls[0]?.[0] ?? '');
    expect(copied).toContain('# Deliverable');
    expect(copied).toContain('REPORT BODY');
    expect(copied).toContain('Must ship');
    expect(copied).not.toContain('FIRST RESPONSE');
    expect(mockSerialize).not.toHaveBeenCalled();
  });

  it('disables copy and export when the bubble has no document or text', () => {
    render(
      <MessageActionBar
        message={createMessage({
          content: [],
          thinking: undefined,
        })}
      />,
    );

    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyAria',
      }),
    ).toBeDisabled();
    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.exportAria',
      }),
    ).toBeDisabled();
  });

  it('exports presentInteractive HTML as .html and hides PDF', async () => {
    mockDownloadTextFile.mockResolvedValue('/tmp/Widget.html');
    mockSerializeForDownload.mockReturnValue('<p>Hi</p>\n');

    const html = [
      '<div id="content-title">A/B:Widget</div>',
      '<script id="raw-data" type="application/json">"<p>Hi</p>\\n"</script>',
    ].join('');

    render(
      <MessageActionBar
        message={createMessage({
          role: 'tool',
          content: [
            {
              type: 'resource',
              resource: {
                uri: 'ui://interactive/xyz',
                mimeType: 'text/html',
                text: html,
              },
            } as MCPContent,
          ],
        })}
      />,
    );

    expect(
      screen.queryByRole('menuitem', {
        name: 'agent.bubble.actionBar.exportPdf',
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', {
        name: 'agent.bubble.actionBar.exportHtml',
      }),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('menuitem', {
        name: 'agent.bubble.actionBar.exportHtml',
      }),
    );

    await waitFor(() => {
      expect(mockDownloadTextFile).toHaveBeenCalledWith({
        fileName: 'A_B_Widget.html',
        content: '<p>Hi</p>\n',
      });
    });
  });
});

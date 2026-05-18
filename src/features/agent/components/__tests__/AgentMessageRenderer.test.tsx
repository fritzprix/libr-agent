import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterAll, beforeEach, describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';
import { toast } from 'sonner';

const downloadMediaFileMock = vi.fn();
const openExternalUrlMock = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/lib/backend/workspace', () => ({
  readLocalFileAsBase64: vi.fn(),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

// Mock contexts and hooks
vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: openExternalUrlMock,
    downloadMediaFile: downloadMediaFileMock,
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      toolCallGroupVisibleCount: 4,
    },
    update: vi.fn(),
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: vi.fn(),
    injectMessages: vi.fn(),
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'test-session', assistant: { id: 'test-assistant' } },
  }),
}));

vi.mock('next-themes', () => ({
  useTheme: () => ({
    resolvedTheme: 'light',
  }),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copied: false,
    copyToClipboard: vi.fn(),
  }),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('AgentMessageRenderer', () => {
  const originalClipboard = navigator.clipboard;
  const originalClipboardItem = globalThis.ClipboardItem;
  const writeClipboardMock = vi.fn();
  const clipboardItemConstructorMock = vi.fn(
    (items: Record<string, Blob>) => items,
  );
  beforeEach(() => {
    writeClipboardMock.mockReset();
    downloadMediaFileMock.mockReset();
    openExternalUrlMock.mockReset();
    vi.mocked(readLocalFileAsBase64).mockReset();
    vi.mocked(toast.success).mockReset();
    vi.mocked(toast.error).mockReset();
    vi.mocked(toast.info).mockReset();

    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        write: writeClipboardMock,
      },
    });

    Object.defineProperty(globalThis, 'ClipboardItem', {
      configurable: true,
      value: clipboardItemConstructorMock,
    });
  });

  afterAll(() => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: originalClipboard,
    });

    Object.defineProperty(globalThis, 'ClipboardItem', {
      configurable: true,
      value: originalClipboardItem,
    });
  });

  function buildImageMessage(content: MCPContent[]): Message {
    return {
      id: 'msg-1',
      sessionId: 'test-session',
      threadId: 'test-session',
      role: 'assistant',
      content,
      createdAt: new Date(),
      updatedAt: new Date(),
    } as Message;
  }

  it('renders text content with code block', async () => {
    const content: MCPContent[] = [
      {
        type: 'text',
        text: 'Here is some code:\n```javascript\nconsole.log("hello");\n```',
      },
    ];

    render(<AgentMessageRenderer content={content} />);

    // Check if code block is rendered
    const codeElement = await screen.findByText('console');
    expect(codeElement).toBeInTheDocument();

    expect(screen.getByText(/hello/)).toBeInTheDocument();
  });

  it('renders tool groups correctly using AgentToolGroupBlock', () => {
    const content: MCPContent[] = [
      {
        type: 'tool_call',
        id: 'call-1',
        name: 'test_tool',
        arguments: '{}',
      },
    ];

    const message = {
      id: 'msg-1',
      role: 'assistant',
      content: [],
    } as unknown as Message;

    render(<AgentMessageRenderer content={content} message={message} />);

    // Check if tool name is rendered
    expect(screen.getByText('test_tool')).toBeInTheDocument();
    // Check if "Tool Executions" header exists (from AgentToolCallGroup)
    expect(screen.getByText('agent.toolGroup.header')).toBeInTheDocument();
  });

  it('lazily materializes file URIs to data URLs for image rendering', async () => {
    vi.mocked(readLocalFileAsBase64).mockResolvedValue('dG9vbC1pbWFnZQ==');
    const content: MCPContent[] = [
      {
        type: 'image',
        uri: 'file:///tmp/tool-output.png',
        mimeType: 'image/png',
      },
    ];
    const message = buildImageMessage(content);

    render(<AgentMessageRenderer content={content} message={message} />);

    const image = await screen.findByAltText('agent.mediaRenderer.imageAlt');
    expect(readLocalFileAsBase64).toHaveBeenCalledWith(
      'test-session',
      'file:///tmp/tool-output.png',
    );
    expect(image).toHaveAttribute(
      'src',
      'data:image/png;base64,dG9vbC1pbWFnZQ==',
    );
  });

  it('shows a visible error state when file-backed image resolution fails', async () => {
    vi.mocked(readLocalFileAsBase64).mockRejectedValue(
      new Error('Permission denied'),
    );
    const content: MCPContent[] = [
      {
        type: 'image',
        uri: 'file:///tmp/tool-output-failure.png',
        mimeType: 'image/png',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    expect(await screen.findByText('agent.mediaRenderer.failedToLoadImage')).toBeInTheDocument();
    expect(screen.getByText('Permission denied')).toBeInTheDocument();
  });

  it('clears the session media cache when the last renderer unmounts', async () => {
    const content: MCPContent[] = [
      {
        type: 'image',
        uri: 'file:///tmp/tool-output-reused.png',
        mimeType: 'image/png',
      },
    ];

    vi.mocked(readLocalFileAsBase64).mockResolvedValueOnce('dG9vbC1pbWFnZQ==');
    const firstRender = render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    expect(await screen.findByAltText('agent.mediaRenderer.imageAlt')).toBeInTheDocument();
    firstRender.unmount();

    vi.mocked(readLocalFileAsBase64).mockReset();
    vi.mocked(readLocalFileAsBase64).mockRejectedValueOnce(
      new Error('Permission denied'),
    );

    render(<AgentMessageRenderer content={content} message={buildImageMessage(content)} />);

    expect(await screen.findByText('agent.mediaRenderer.failedToLoadImage')).toBeInTheDocument();
    expect(screen.getByText('Permission denied')).toBeInTheDocument();
  });

  it('copies inline image data to the clipboard without refetching the image source', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    const content: MCPContent[] = [
      {
        type: 'image',
        data: 'dG9vbC1pbWFnZQ==',
        mimeType: 'image/png',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    fireEvent.click(await screen.findByLabelText('agent.mediaRenderer.copyAria'));

    await waitFor(() => {
      expect(writeClipboardMock).toHaveBeenCalledTimes(1);
    });
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(clipboardItemConstructorMock).toHaveBeenCalledTimes(1);

    const clipboardPayload = clipboardItemConstructorMock.mock.calls[0]?.[0] as
      | Record<string, Blob>
      | undefined;
    expect(clipboardPayload).toBeDefined();
    expect(clipboardPayload?.['image/png']).toBeInstanceOf(Blob);
    expect(vi.mocked(toast.success)).toHaveBeenCalledWith('agent.mediaRenderer.copySuccess');

    fetchSpy.mockRestore();
  });

  it('downloads file-backed images via the native media download command', async () => {
    vi.mocked(readLocalFileAsBase64).mockResolvedValue('dG9vbC1pbWFnZQ==');
    downloadMediaFileMock.mockResolvedValue('File downloaded successfully');
    const content: MCPContent[] = [
      {
        type: 'image',
        uri: 'file:///tmp/tool-output.png',
        mimeType: 'image/png',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    fireEvent.click(await screen.findByLabelText('agent.mediaRenderer.downloadAria'));

    await waitFor(() => {
      expect(downloadMediaFileMock).toHaveBeenCalledWith({
        sessionId: 'test-session',
        fileName: 'tool-output.png',
        mimeType: 'image/png',
        dataBase64: 'dG9vbC1pbWFnZQ==',
        fileUrl: undefined,
      });
    });
    expect(vi.mocked(toast.success)).toHaveBeenCalledWith(
      'File downloaded successfully',
    );
  });

  it('downloads inline data-url images through the native media download command', async () => {
    downloadMediaFileMock.mockResolvedValue('File downloaded successfully');
    const content: MCPContent[] = [
      {
        type: 'image',
        data: 'data:image/png;base64,dG9vbC1pbWFnZQ==',
        mimeType: 'image/png',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    fireEvent.click(await screen.findByLabelText('agent.mediaRenderer.downloadAria'));

    await waitFor(() => {
      expect(downloadMediaFileMock).toHaveBeenCalledWith({
        sessionId: 'test-session',
        fileName: expect.stringMatching(/^image-\d+\.png$/),
        mimeType: 'image/png',
        dataBase64: 'dG9vbC1pbWFnZQ==',
        fileUrl: undefined,
      });
    });
  });

  it('blocks unsafe URLs in resource-link click handler', async () => {
    const content: MCPContent[] = [
      {
        type: 'resource_link',
        uri: 'javascript:alert(1)',
        name: 'Malicious Link',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    const link = await screen.findByText('Malicious Link');
    fireEvent.click(link);

    expect(openExternalUrlMock).not.toHaveBeenCalled();
  });

  it('renders unsafe protocols in Markdown as inert text', async () => {
    const content: MCPContent[] = [
      {
        type: 'text',
        text: '[Malicious Link](javascript:alert("XSS"))',
      },
    ];

    render(
      <AgentMessageRenderer content={content} message={buildImageMessage(content)} />,
    );

    // It should render the text but not as an <a> tag
    const linkText = await screen.findByText('Malicious Link');
    expect(linkText.tagName).toBe('SPAN');
    expect(linkText).toHaveClass('text-muted-foreground');
    expect(screen.queryByRole('link', { name: 'Malicious Link' })).not.toBeInTheDocument();
  });
});

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';
import { MessageActionBar } from '../MessageActionBar';

const mockCopyToClipboard = vi.fn();
const mockSerialize = vi.fn();
const mockSerializeForDownload = vi.fn();
const mockDownloadTextFile = vi.fn();
const mockDownloadTextPdf = vi.fn();

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

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

vi.mock('@/features/agent/lib/message-serialization', () => ({
  serializeMessageForClipboard: (...args: unknown[]) => mockSerialize(...args),
  serializeMessageForDownload: (...args: unknown[]) =>
    mockSerializeForDownload(...args),
  buildMessageExportFilename: (_message: Message, extension: string) =>
    `export.${extension}`,
}));

vi.mock('@/lib/backend', () => ({
  downloadTextFile: (...args: unknown[]) => mockDownloadTextFile(...args),
  downloadTextPdf: (...args: unknown[]) => mockDownloadTextPdf(...args),
  openPathWithDefaultApp: vi.fn(),
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
    mockSerialize.mockReturnValue('## Assistant\n\nHello');
    mockSerializeForDownload.mockReturnValue('## Answer\n\n- point one');
    mockCopyToClipboard.mockResolvedValue(undefined);
    mockDownloadTextFile.mockResolvedValue('/tmp/message.md');
    mockDownloadTextPdf.mockResolvedValue('/tmp/message.pdf');
  });

  it('copies the full message when the primary copy button is clicked', async () => {
    render(<MessageActionBar message={createMessage()} />);

    fireEvent.click(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyFullAria',
      }),
    );

    await waitFor(() => {
      expect(mockSerialize).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'msg-1' }),
        expect.objectContaining({ mode: 'full' }),
      );
      expect(mockCopyToClipboard).toHaveBeenCalledWith(
        '## Assistant\n\nHello',
      );
    });
  });

  it('keeps the primary copy control visible without hover', () => {
    render(<MessageActionBar message={createMessage()} />);

    const copyButton = screen.getByRole('button', {
      name: 'agent.bubble.actionBar.copyFullAria',
    });

    expect(copyButton).toBeVisible();
    expect(copyButton.className).not.toMatch(/opacity-0/);
  });

  it('exposes icon-only flat actions with aria labels', () => {
    render(<MessageActionBar message={createMessage()} />);

    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyFullAria',
      }),
    ).toBeVisible();
    expect(
      screen.getByRole('button', {
        name: 'agent.bubble.actionBar.copyTextAria',
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
});

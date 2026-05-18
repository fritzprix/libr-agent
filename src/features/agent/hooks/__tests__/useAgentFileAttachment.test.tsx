import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentFileAttachment } from '../useAgentFileAttachment';

const mockResourceAttachment = vi.hoisted(() => ({
  pendingFiles: [],
  addPendingFiles: vi.fn(),
  updatePendingFile: vi.fn(),
  commitPendingFiles: vi.fn(),
  removeFile: vi.fn(),
  clearPendingFiles: vi.fn(),
  refetchSessionFiles: vi.fn(),
  isLoading: false,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      options?.error
        ? `${key}:${String(options.error)}`
        : key,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: {
      id: 'session-1',
    },
  }),
}));

vi.mock('@/features/agent/hooks/useAgentResourceAttachment', () => ({
  useAgentResourceAttachment: () => mockResourceAttachment,
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      system: {
        maxFileUploadSizeMB: 1,
      },
    },
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    registerDroppedFiles: vi.fn(),
    readDroppedFile: vi.fn(),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
  },
}));

describe('useAgentFileAttachment', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockResourceAttachment.addPendingFiles.mockImplementation(
      (
        files: Array<{
          filename: string;
          status: string;
        }>,
      ) =>
        files.map((file, index) => ({
          pendingId: `pending-${index}`,
          filename: file.filename,
          status: file.status,
        })),
    );
  });

  it('assigns stable generated names to pasted clipboard images without filenames', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(1234567890);

    const { result } = renderHook(() => useAgentFileAttachment());

    const unnamedPng = new File(['png-bytes'], '', { type: 'image/png' });
    const unnamedGif = new File(['gif-bytes'], '', { type: 'image/gif' });

    await act(async () => {
      await result.current.attachFiles([unnamedPng, unnamedGif]);
    });

    expect(mockResourceAttachment.addPendingFiles).toHaveBeenCalledWith([
      {
        url: '',
        filename: 'pasted-image-1234567890.png',
        mimeType: 'image/png',
        status: 'processing',
      },
      {
        url: '',
        filename: 'pasted-image-1234567890-2.gif',
        mimeType: 'image/gif',
        status: 'processing',
      },
    ]);

    expect(mockResourceAttachment.updatePendingFile).toHaveBeenNthCalledWith(
      1,
      'pending-0',
      expect.objectContaining({
        size: unnamedPng.size,
        status: 'pending',
        file: expect.objectContaining({
          name: 'pasted-image-1234567890.png',
          type: 'image/png',
        }),
      }),
    );
    expect(mockResourceAttachment.updatePendingFile).toHaveBeenNthCalledWith(
      2,
      'pending-1',
      expect.objectContaining({
        size: unnamedGif.size,
        status: 'pending',
        file: expect.objectContaining({
          name: 'pasted-image-1234567890-2.gif',
          type: 'image/gif',
        }),
      }),
    );
  });

  it('removes oversized files after optimistic placeholder creation', async () => {
    const { result } = renderHook(() => useAgentFileAttachment());
    const oversizedFile = new File(
      [new Uint8Array(1024 * 1024 + 1)],
      'huge-image.png',
      { type: 'image/png' },
    );

    await act(async () => {
      await result.current.attachFiles([oversizedFile]);
    });

    expect(mockResourceAttachment.removeFile).toHaveBeenCalledWith(
      expect.objectContaining({
        pendingId: 'pending-0',
        filename: 'huge-image.png',
      }),
    );
    expect(mockResourceAttachment.updatePendingFile).not.toHaveBeenCalled();
  });
});

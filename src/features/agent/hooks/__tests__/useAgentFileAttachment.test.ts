import { vi, describe, it, expect, beforeEach, afterEach, type Mock } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAgentFileAttachment } from '../useAgentFileAttachment';

// Mocks
const mockUseSettings = {
  value: {
    system: {
      maxFileUploadSizeMB: 50,
      workspaceCapacityMB: 10,
    },
  },
};

const mockUseAgentSessionState = {
  session: { id: 'test-session-id' },
};

const mockUseAgentResourceAttachment = {
  pendingFiles: [],
  addPendingFiles: vi.fn(),
  commitPendingFiles: vi.fn(),
  removeFile: vi.fn(),
  clearPendingFiles: vi.fn(),
  isLoading: false,
  refetchSessionFiles: vi.fn(),
};

const mockUseRustBackend = {
  readDroppedFile: vi.fn(),
  registerDroppedFiles: vi.fn(),
};

const mockValidateFileSize = vi.fn();
const mockCreateFileSizeErrorMessage = vi.fn();

// Setup mocks
vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => mockUseSettings,
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => mockUseAgentSessionState,
}));

vi.mock('@/features/agent/hooks/useAgentResourceAttachment', () => ({
  useAgentResourceAttachment: () => mockUseAgentResourceAttachment,
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => mockUseRustBackend,
}));

vi.mock('@/lib/workspace-sync-service', () => ({
  validateFileSize: (...args: unknown[]) => mockValidateFileSize(...args),
  createFileSizeErrorMessage: (...args: unknown[]) => mockCreateFileSizeErrorMessage(...args),
}));

describe('useAgentFileAttachment', () => {
  const originalAlert = window.alert;

  beforeEach(() => {
    vi.clearAllMocks();
    window.alert = vi.fn();

    // Default mock implementations
    mockUseAgentSessionState.session = { id: 'test-session-id' };
    mockUseRustBackend.readDroppedFile.mockResolvedValue([1, 2, 3]); // Mock file content
    mockUseRustBackend.registerDroppedFiles.mockResolvedValue(undefined);
    mockValidateFileSize.mockReturnValue(true);
    mockCreateFileSizeErrorMessage.mockImplementation((name: string) => `File ${name} is too big`);
  });

  afterEach(() => {
    window.alert = originalAlert;
  });

  describe('getMimeType', () => {
    it('should return correct mime types for supported extensions', () => {
      const { result } = renderHook(() => useAgentFileAttachment());

      expect(result.current.getMimeType('test.txt')).toBe('text/plain');
      expect(result.current.getMimeType('test.md')).toBe('text/markdown');
      expect(result.current.getMimeType('test.json')).toBe('application/json');
      expect(result.current.getMimeType('test.pdf')).toBe('application/pdf');
      expect(result.current.getMimeType('test.docx')).toBe('application/vnd.openxmlformats-officedocument.wordprocessingml.document');
      expect(result.current.getMimeType('test.xlsx')).toBe('application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
    });

    it('should return application/octet-stream for unknown extensions', () => {
      const { result } = renderHook(() => useAgentFileAttachment());
      expect(result.current.getMimeType('test.unknown')).toBe('application/octet-stream');
      expect(result.current.getMimeType('test')).toBe('application/octet-stream');
    });
  });

  describe('processFileDrop', () => {
    it('should alert if no session available', async () => {
      mockUseAgentSessionState.session = null as unknown as { id: string };
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(['/path/to/file.txt']);
      });

      expect(window.alert).toHaveBeenCalledWith('Cannot attach file: session not available.');
      expect(mockUseRustBackend.registerDroppedFiles).not.toHaveBeenCalled();
    });

    it('should register and read dropped files', async () => {
      const filePaths = ['/path/to/file1.txt', '/path/to/file2.md'];
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(filePaths);
      });

      expect(mockUseRustBackend.registerDroppedFiles).toHaveBeenCalledWith(filePaths);
      expect(mockUseRustBackend.readDroppedFile).toHaveBeenCalledTimes(2);
      expect(mockUseRustBackend.readDroppedFile).toHaveBeenCalledWith(filePaths[0]);
      expect(mockUseRustBackend.readDroppedFile).toHaveBeenCalledWith(filePaths[1]);

      expect(mockUseAgentResourceAttachment.addPendingFiles).toHaveBeenCalledTimes(1);
      const pendingFiles = (mockUseAgentResourceAttachment.addPendingFiles as Mock).mock.calls[0][0];
      expect(pendingFiles).toHaveLength(2);
      expect(pendingFiles[0].filename).toBe('file1.txt');
      expect(pendingFiles[0].mimeType).toBe('text/plain');
      expect(pendingFiles[1].filename).toBe('file2.md');
      expect(pendingFiles[1].mimeType).toBe('text/markdown');
    });

    it('should validate file size and alert on failure', async () => {
      mockValidateFileSize.mockReturnValue(false);
      const filePaths = ['/path/to/large_file.txt'];
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(filePaths);
      });

      expect(mockValidateFileSize).toHaveBeenCalled();
      expect(mockCreateFileSizeErrorMessage).toHaveBeenCalled();
      expect(window.alert).toHaveBeenCalledWith(expect.stringContaining('File large_file.txt is too big'));
      expect(mockUseAgentResourceAttachment.addPendingFiles).not.toHaveBeenCalled();
    });

    it('should handle errors during file processing gracefully', async () => {
      mockUseRustBackend.readDroppedFile.mockRejectedValue(new Error('Read failed'));
      const filePaths = ['/path/to/error_file.txt'];
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(filePaths);
      });

      expect(window.alert).toHaveBeenCalledWith(expect.stringContaining('Error processing file'));
      expect(mockUseAgentResourceAttachment.addPendingFiles).not.toHaveBeenCalled();
    });

    it('should alert and stop early when registerDroppedFiles throws', async () => {
      mockUseRustBackend.registerDroppedFiles.mockRejectedValue(new Error('Register failed'));
      const filePaths = ['/path/to/file.txt'];
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(filePaths);
      });

      expect(window.alert).toHaveBeenCalledWith('Failed to validate dropped files. Please try again.');
      expect(mockUseRustBackend.readDroppedFile).not.toHaveBeenCalled();
      expect(mockUseAgentResourceAttachment.addPendingFiles).not.toHaveBeenCalled();
    });

    it('should alert when batch addPendingFiles throws after files are prepared', async () => {
      (mockUseAgentResourceAttachment.addPendingFiles as Mock).mockImplementation(() => {
        throw new Error('Batch upload failed');
      });
      const filePaths = ['/path/to/file.txt'];
      const { result } = renderHook(() => useAgentFileAttachment());

      await act(async () => {
        await result.current.processFileDrop(filePaths);
      });

      expect(window.alert).toHaveBeenCalledWith(
        expect.stringContaining('Batch upload failed'),
      );
    });
  });

  describe('handleFileAttachment', () => {
    it('should process file input change', async () => {
      const { result } = renderHook(() => useAgentFileAttachment());

      const file = new File(['content'], 'test.txt', { type: 'text/plain' });
      const event = {
        target: {
          files: [file],
          value: 'C:\\fakepath\\test.txt',
        },
      } as unknown as React.ChangeEvent<HTMLInputElement>;

      await act(async () => {
        await result.current.handleFileAttachment(event);
      });

      expect(mockValidateFileSize).toHaveBeenCalledWith(file, expect.any(Number));
      expect(mockUseAgentResourceAttachment.addPendingFiles).toHaveBeenCalledTimes(1);

      const pendingFiles = (mockUseAgentResourceAttachment.addPendingFiles as Mock).mock.calls[0][0];
      expect(pendingFiles).toHaveLength(1);
      expect(pendingFiles[0].file).toBe(file);

      // Should clear input value
      expect(event.target.value).toBe('');
    });

    it('should alert if no session available', async () => {
      mockUseAgentSessionState.session = null as unknown as { id: string };
      const { result } = renderHook(() => useAgentFileAttachment());

      const file = new File(['content'], 'test.txt', { type: 'text/plain' });
      const event = {
        target: {
          files: [file],
        },
      } as unknown as React.ChangeEvent<HTMLInputElement>;

      await act(async () => {
        await result.current.handleFileAttachment(event);
      });

      expect(window.alert).toHaveBeenCalledWith('Cannot attach file: session not available.');
      expect(mockUseAgentResourceAttachment.addPendingFiles).not.toHaveBeenCalled();
    });

    it('should validate file size and alert on failure', async () => {
      mockValidateFileSize.mockReturnValue(false);
      const { result } = renderHook(() => useAgentFileAttachment());

      const file = new File(['content'], 'large.txt', { type: 'text/plain' });
      const event = {
        target: {
          files: [file],
          value: 'path',
        },
      } as unknown as React.ChangeEvent<HTMLInputElement>;

      await act(async () => {
        await result.current.handleFileAttachment(event);
      });

      expect(window.alert).toHaveBeenCalledWith(expect.stringContaining('File large.txt is too big'));
      expect(mockUseAgentResourceAttachment.addPendingFiles).not.toHaveBeenCalled();
    });

    it('should alert and clear input when addPendingFiles throws', async () => {
      (mockUseAgentResourceAttachment.addPendingFiles as Mock).mockImplementation(() => {
        throw new Error('Test addPendingFiles error');
      });
      const { result } = renderHook(() => useAgentFileAttachment());

      const file = new File(['content'], 'error.txt', { type: 'text/plain' });
      const event = {
        target: {
          files: [file],
          value: 'C:\\fakepath\\error.txt',
        },
      } as unknown as React.ChangeEvent<HTMLInputElement>;

      await act(async () => {
        await result.current.handleFileAttachment(event);
      });

      expect(window.alert).toHaveBeenCalledWith(
        expect.stringContaining('Test addPendingFiles error'),
      );
      expect(event.target.value).toBe('');
    });
  });

  describe('validateFiles', () => {
    it('should return true for any file path (current implementation)', () => {
      const { result } = renderHook(() => useAgentFileAttachment());

      expect(result.current.validateFiles(['test.txt'])).toBe(true);
      expect(result.current.validateFiles(['test.exe'])).toBe(true); // Assuming allow-all policy
    });
  });
});

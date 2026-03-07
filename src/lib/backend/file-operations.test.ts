import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  readDroppedFile,
  registerDroppedFiles,
  writeFile,
} from './file-operations';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('backend/file-operations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should read dropped file via read_dropped_file', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([1, 2, 3]);
    const res = await readDroppedFile('/path/to/file');
    expect(safeInvoke).toHaveBeenCalledWith('read_dropped_file', { filePath: '/path/to/file' });
    expect(res).toEqual([1, 2, 3]);
  });

  it('should register dropped files via register_dropped_files', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
    await registerDroppedFiles(['/path1', '/path2']);
    expect(safeInvoke).toHaveBeenCalledWith('register_dropped_files', { paths: ['/path1', '/path2'] });
  });

  it('should write file via write_file', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
    await writeFile('/path/to/file', [1, 2, 3]);
    expect(safeInvoke).toHaveBeenCalledWith('write_file', { filePath: '/path/to/file', content: [1, 2, 3] });
  });
});

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listWorkspaceFiles,
  workspaceWriteFile,
  openWorkspaceFileWithDefaultApp,
  openWorkspaceInExplorer,
  openWorkspaceInTerminal,
  getWorkspaceOverride,
  setWorkspaceOverride,
  cancelWorkspaceOverride,
  getWorkspaceDir,
  readLocalFileAsBase64,
  listWorkspaceFilePaths,
  listWorkspaceFilePathsForPath,
} from './workspace';
import { safeInvoke } from './core';
import type { WorkspaceFileItem } from './types';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('workspace backend wrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const mockFileItem: WorkspaceFileItem = {
    name: 'test.txt',
    path: '/path/to/test.txt',
    size: 1024,
    modified: '2023-01-01T00:00:00Z',
    isDirectory: false,
  };

  it('listWorkspaceFiles calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([mockFileItem]);

    const result = await listWorkspaceFiles('src', 'session-1');

    expect(safeInvoke).toHaveBeenCalledWith('list_workspace_files', {
      path: 'src',
      sessionId: 'session-1',
    });
    expect(result).toEqual([mockFileItem]);
  });

  it('listWorkspaceFiles calls safeInvoke with null arguments when omitted', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([mockFileItem]);

    const result = await listWorkspaceFiles();

    expect(safeInvoke).toHaveBeenCalledWith('list_workspace_files', {
      path: null,
      sessionId: null,
    });
    expect(result).toEqual([mockFileItem]);
  });

  it('workspaceWriteFile calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await workspaceWriteFile('test.txt', [1, 2, 3], 'session-1');

    expect(safeInvoke).toHaveBeenCalledWith('workspace_write_file', {
      filePath: 'test.txt',
      content: [1, 2, 3],
      sessionId: 'session-1',
    });
  });

  it('workspaceWriteFile calls safeInvoke with null sessionId when omitted', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await workspaceWriteFile('test.txt', [1, 2, 3]);

    expect(safeInvoke).toHaveBeenCalledWith('workspace_write_file', {
      filePath: 'test.txt',
      content: [1, 2, 3],
      sessionId: null,
    });
  });

  it('openWorkspaceFileWithDefaultApp calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await openWorkspaceFileWithDefaultApp('test.txt', 'session-1');

    expect(safeInvoke).toHaveBeenCalledWith(
      'open_workspace_file_with_default_app',
      {
        filePath: 'test.txt',
        sessionId: 'session-1',
      },
    );
  });

  it('openWorkspaceInExplorer calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await openWorkspaceInExplorer('session-1');

    expect(safeInvoke).toHaveBeenCalledWith('open_workspace_in_explorer', {
      sessionId: 'session-1',
    });
  });

  it('openWorkspaceInTerminal calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await openWorkspaceInTerminal('session-1');

    expect(safeInvoke).toHaveBeenCalledWith('open_workspace_in_terminal', {
      sessionId: 'session-1',
    });
  });

  it('getWorkspaceOverride calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('/custom/path');

    const result = await getWorkspaceOverride('session-1');

    expect(safeInvoke).toHaveBeenCalledWith('get_workspace_override', {
      sessionId: 'session-1',
    });
    expect(result).toBe('/custom/path');
  });

  it('setWorkspaceOverride calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await setWorkspaceOverride('session-1', '/custom/path');

    expect(safeInvoke).toHaveBeenCalledWith('set_workspace_override', {
      sessionId: 'session-1',
      overridePath: '/custom/path',
    });
  });

  it('cancelWorkspaceOverride calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await cancelWorkspaceOverride('session-1');

    expect(safeInvoke).toHaveBeenCalledWith('cancel_workspace_override', {
      sessionId: 'session-1',
    });
  });

  it('getWorkspaceDir calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('/workspace/session-1');

    const result = await getWorkspaceDir('session-1');

    expect(safeInvoke).toHaveBeenCalledWith('get_workspace_dir', {
      sessionId: 'session-1',
    });
    expect(result).toBe('/workspace/session-1');
  });

  it('readLocalFileAsBase64 calls safeInvoke with session-scoped arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('Zm9v');

    const result = await readLocalFileAsBase64(
      'session-1',
      'file:///workspace/session-1/image.png',
    );

    expect(safeInvoke).toHaveBeenCalledWith('read_local_file_as_base64', {
      sessionId: 'session-1',
      fileUrl: 'file:///workspace/session-1/image.png',
    });
    expect(result).toBe('Zm9v');
  });

  it('listWorkspaceFilePaths calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(['file1.txt', 'dir1/file2.txt']);

    const result = await listWorkspaceFilePaths('session-1', 2);

    expect(safeInvoke).toHaveBeenCalledWith('list_workspace_file_paths', {
      sessionId: 'session-1',
      maxDepth: 2,
    });
    expect(result).toEqual(['file1.txt', 'dir1/file2.txt']);
  });

  it('listWorkspaceFilePathsForPath calls safeInvoke with correct arguments', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(['file1.txt', 'dir1/file2.txt']);

    const result = await listWorkspaceFilePathsForPath('/tmp/workspace', 3);

    expect(safeInvoke).toHaveBeenCalledWith(
      'list_workspace_file_paths_for_path',
      {
        workspacePath: '/tmp/workspace',
        maxDepth: 3,
      },
    );
    expect(result).toEqual(['file1.txt', 'dir1/file2.txt']);
  });
});

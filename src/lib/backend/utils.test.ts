import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  getAppLogsDir,
  backupCurrentLog,
  clearCurrentLog,
  listLogFiles,
  openExternalUrl,
  downloadWorkspaceFile,
  exportAndDownloadZip,
  getServiceContext,
  greet,
  restartApp,
} from './utils';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('backend/utils', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Log Management', () => {
    it('should get app logs dir via get_app_logs_dir', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('/fake/logs/dir');
      const res = await getAppLogsDir();
      expect(safeInvoke).toHaveBeenCalledWith('get_app_logs_dir');
      expect(res).toBe('/fake/logs/dir');
    });

    it('should backup current log via backup_current_log', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('/fake/logs/dir/backup.log');
      const res = await backupCurrentLog();
      expect(safeInvoke).toHaveBeenCalledWith('backup_current_log');
      expect(res).toBe('/fake/logs/dir/backup.log');
    });

    it('should clear current log via clear_current_log', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      await clearCurrentLog();
      expect(safeInvoke).toHaveBeenCalledWith('clear_current_log');
    });

    it('should list log files via list_log_files', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(['log1.txt', 'log2.txt']);
      const res = await listLogFiles();
      expect(safeInvoke).toHaveBeenCalledWith('list_log_files');
      expect(res).toEqual(['log1.txt', 'log2.txt']);
    });
  });

  describe('External URL Handling', () => {
    it('should open external url via open_external_url', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      await openExternalUrl('https://example.com');
      expect(safeInvoke).toHaveBeenCalledWith('open_external_url', { url: 'https://example.com' });
    });
  });

  describe('File Download Operations', () => {
    it('should download workspace file via download_workspace_file', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('download started');
      const res = await downloadWorkspaceFile('file.txt', 'session-123');
      expect(safeInvoke).toHaveBeenCalledWith('download_workspace_file', { filePath: 'file.txt', sessionId: 'session-123' });
      expect(res).toBe('download started');
    });

    it('should export and download zip via export_and_download_zip', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('export started');
      const res = await exportAndDownloadZip(['file1.txt', 'file2.txt'], 'my-package', 'session-123');
      expect(safeInvoke).toHaveBeenCalledWith('export_and_download_zip', { files: ['file1.txt', 'file2.txt'], packageName: 'my-package', sessionId: 'session-123' });
      expect(res).toBe('export started');
    });
  });

  describe('Service Context', () => {
    it('should get service context via get_service_context with options', async () => {
      const mockContext = { id: 'test' };
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockContext);
      const res = await getServiceContext('server-123', { sessionId: 'session-456' });
      expect(safeInvoke).toHaveBeenCalledWith('get_service_context', { serverId: 'server-123', options: { sessionId: 'session-456' } });
      expect(res).toEqual(mockContext);
    });

    it('should get service context via get_service_context without options', async () => {
      const mockContext = { id: 'test' };
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockContext);
      const res = await getServiceContext('server-123');
      expect(safeInvoke).toHaveBeenCalledWith('get_service_context', { serverId: 'server-123', options: undefined });
      expect(res).toEqual(mockContext);
    });
  });

  describe('Miscellaneous', () => {
    it('should greet via greet', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('Hello, test');
      const res = await greet('test');
      expect(safeInvoke).toHaveBeenCalledWith('greet', { name: 'test' });
      expect(res).toBe('Hello, test');
    });

    it('should restart app via restart_app', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      await restartApp();
      expect(safeInvoke).toHaveBeenCalledWith('restart_app');
    });
  });
});

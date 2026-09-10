import { describe, it, expect, vi, beforeEach } from 'vitest';
import { exportSessionFile } from './session-export';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('backend/session-export', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes export_session_file for markdown', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('/tmp/session.md');
    const res = await exportSessionFile({
      sessionId: 'session-123',
      format: 'markdown',
    });
    expect(safeInvoke).toHaveBeenCalledWith('export_session_file', {
      sessionId: 'session-123',
      format: 'markdown',
    });
    expect(res).toBe('/tmp/session.md');
  });

  it('invokes export_session_file for atif', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('/tmp/session_trajectory.json');
    const res = await exportSessionFile({
      sessionId: 'session-123',
      format: 'atif',
    });
    expect(safeInvoke).toHaveBeenCalledWith('export_session_file', {
      sessionId: 'session-123',
      format: 'atif',
    });
    expect(res).toBe('/tmp/session_trajectory.json');
  });
});

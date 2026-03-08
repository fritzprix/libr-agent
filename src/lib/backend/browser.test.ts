import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  createBrowserSession,
  closeBrowserSession,
  listBrowserSessions,
  navigateToUrl,
} from './browser';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('backend/browser', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create browser session via create_browser_session', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({ session_id: 'sess-1', message: 'ok' });
    const res = await createBrowserSession({ url: 'http://test' });
    expect(safeInvoke).toHaveBeenCalledWith('create_browser_session', { url: 'http://test' });
    expect(res).toEqual({ session_id: 'sess-1', message: 'ok' });
  });

  it('should close browser session via close_browser_session', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
    await closeBrowserSession('sess-1');
    expect(safeInvoke).toHaveBeenCalledWith('close_browser_session', { sessionId: 'sess-1' });
  });

  it('should list browser sessions via list_browser_sessions', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([{ id: 'sess-1' }]);
    const res = await listBrowserSessions();
    expect(safeInvoke).toHaveBeenCalledWith('list_browser_sessions');
    expect(res).toEqual([{ id: 'sess-1' }]);
  });

  it('should navigate to url via navigate_to_url', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');
    const res = await navigateToUrl('sess-1', 'http://test2');
    expect(safeInvoke).toHaveBeenCalledWith('navigate_to_url', { sessionId: 'sess-1', url: 'http://test2' });
    expect(res).toBe('success');
  });
});

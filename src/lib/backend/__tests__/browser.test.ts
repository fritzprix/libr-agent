import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  createBrowserSession,
  closeBrowserSession,
  listBrowserSessions,
  navigateToUrl
} from '../browser';
import { safeInvoke } from '../core';

vi.mock('../core', () => ({
  safeInvoke: vi.fn(),
}));

describe('browser backend commands', () => {
  beforeEach(() => {
    vi.mocked(safeInvoke).mockClear();
  });

  it('createBrowserSession calls correct backend command', async () => {
    const mockResponse = { session_id: '123', message: 'success' };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const params = { url: 'https://example.com', title: 'Example' };
    const result = await createBrowserSession(params);

    expect(safeInvoke).toHaveBeenCalledWith('create_browser_session', params);
    expect(result).toBe(mockResponse);
  });

  it('closeBrowserSession calls correct backend command', async () => {
    await closeBrowserSession('session-123');

    expect(safeInvoke).toHaveBeenCalledWith('close_browser_session', { sessionId: 'session-123' });
  });

  it('listBrowserSessions calls correct backend command', async () => {
    const mockSessions = [{ id: '123', url: 'https://example.com' }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockSessions);

    const result = await listBrowserSessions();

    expect(safeInvoke).toHaveBeenCalledWith('list_browser_sessions');
    expect(result).toBe(mockSessions);
  });

  it('navigateToUrl calls correct backend command', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce('success');

    const result = await navigateToUrl('session-123', 'https://example.com');

    expect(safeInvoke).toHaveBeenCalledWith('navigate_to_url', {
      sessionId: 'session-123',
      url: 'https://example.com'
    });
    expect(result).toBe('success');
  });
});

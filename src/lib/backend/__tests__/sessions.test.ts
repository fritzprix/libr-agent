import { describe, it, expect, vi, beforeEach } from 'vitest';
import { removeSession, deleteAttachments, clearAllSessions, factoryReset } from '../sessions';
import { safeInvoke } from '../core';

vi.mock('../core', () => ({
  safeInvoke: vi.fn(),
}));

describe('sessions backend commands', () => {
  beforeEach(() => {
    vi.mocked(safeInvoke).mockClear();
  });

  it('removeSession calls correct backend command', async () => {
    await removeSession('test-session-id');
    expect(safeInvoke).toHaveBeenCalledWith('remove_session', { sessionId: 'test-session-id' });
  });

  it('deleteAttachments calls correct backend command', async () => {
    await deleteAttachments('test-session-id');
    expect(safeInvoke).toHaveBeenCalledWith('delete_attachments', { sessionId: 'test-session-id' });
  });

  it('clearAllSessions calls correct backend command', async () => {
    await clearAllSessions();
    expect(safeInvoke).toHaveBeenCalledWith('agent_clear_all_sessions');
  });

  it('factoryReset calls correct backend command', async () => {
    await factoryReset();
    expect(safeInvoke).toHaveBeenCalledWith('agent_factory_reset');
  });
});

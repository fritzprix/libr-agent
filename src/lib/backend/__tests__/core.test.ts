import { describe, it, expect, vi, beforeEach } from 'vitest';
import { safeInvoke } from '../core';
import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockDebug = vi.fn();
const mockError = vi.fn();

vi.mock('@/lib/logger', () => {
  return {
    getLogger: vi.fn(() => ({
      debug: vi.fn((...args) => mockDebug(...args)),
      error: vi.fn((...args) => mockError(...args)),
    })),
  };
});

describe('core backend module', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockClear();
    vi.mocked(getLogger).mockClear();
    mockDebug.mockClear();
    mockError.mockClear();
  });

  it('safeInvoke calls invoke successfully and logs debug', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('success result');
    const result = await safeInvoke('test_command', { arg1: 'value' });

    expect(result).toBe('success result');
    expect(invoke).toHaveBeenCalledWith('test_command', { arg1: 'value' });
    expect(mockDebug).toHaveBeenCalledWith('invoke', { cmd: 'test_command', args: { arg1: 'value' } });
  });

  it('safeInvoke passes empty object as default args', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('success');
    await safeInvoke('test_command');
    expect(invoke).toHaveBeenCalledWith('test_command', {});
    expect(mockDebug).toHaveBeenCalledWith('invoke', { cmd: 'test_command', args: undefined });
  });

  it('safeInvoke handles errors and logs error', async () => {
    const error = new Error('test error');
    vi.mocked(invoke).mockRejectedValueOnce(error);

    await expect(safeInvoke('test_command')).rejects.toThrow('test error');
    expect(mockError).toHaveBeenCalledWith('invoke failed', { cmd: 'test_command', err: error });
  });
});

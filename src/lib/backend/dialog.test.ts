import { describe, it, expect, vi, beforeEach } from 'vitest';

import { safeInvoke } from './core';
import { openDialog } from './dialog';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

const pluginOpen = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (...args: unknown[]) => pluginOpen(...args),
}));

describe('backend/dialog (E2E picker override, #2903)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('opens the real dialog when no override is armed', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(null);
    pluginOpen.mockResolvedValueOnce('/picked/by/human.txt');

    const result = await openDialog({ multiple: false, title: 'pick' });

    expect(safeInvoke).toHaveBeenCalledWith('e2e_take_picker_override');
    expect(pluginOpen).toHaveBeenCalledWith({ multiple: false, title: 'pick' });
    expect(result).toBe('/picked/by/human.txt');
  });

  it('resolves an armed path override without touching the plugin', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({ action: 'path', path: '/tmp/e2e.txt' });

    const result = await openDialog({ directory: true });

    expect(pluginOpen).not.toHaveBeenCalled();
    expect(result).toBe('/tmp/e2e.txt');
  });

  it('wraps the path in an array when multiple is set', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({ action: 'path', path: '/tmp/e2e.txt' });

    const result = await openDialog({ multiple: true });

    expect(result).toEqual(['/tmp/e2e.txt']);
  });

  it('resolves a cancel override as null without touching the plugin', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({ action: 'cancel' });

    const result = await openDialog({ title: 'pick' });

    expect(pluginOpen).not.toHaveBeenCalled();
    expect(result).toBeNull();
  });

  it('rejects a malformed override payload', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({ action: 'surprise' });

    await expect(openDialog()).rejects.toThrow();
    expect(pluginOpen).not.toHaveBeenCalled();
  });
});

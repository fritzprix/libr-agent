import { describe, it, expect, vi, beforeEach } from 'vitest';
import { checkDockerHealth, waitForDockerReady } from './dockerHealth';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('dockerHealth', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('checkDockerHealth calls safeInvoke with correct command', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    await checkDockerHealth();

    expect(safeInvoke).toHaveBeenCalledWith('check_docker_health');
  });

  it('waitForDockerReady returns true on first successful check', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

    const result = await waitForDockerReady({
      maxAttempts: 3,
      intervalMs: 10,
      sleep: async () => {},
    });

    expect(result).toBe(true);
    expect(safeInvoke).toHaveBeenCalledTimes(1);
  });

  it('waitForDockerReady retries until success', async () => {
    vi.mocked(safeInvoke)
      .mockRejectedValueOnce(new Error('not ready'))
      .mockResolvedValueOnce(undefined);

    const result = await waitForDockerReady({
      maxAttempts: 3,
      intervalMs: 10,
      sleep: async () => {},
    });

    expect(result).toBe(true);
    expect(safeInvoke).toHaveBeenCalledTimes(2);
  });

  it('waitForDockerReady returns false after max attempts', async () => {
    vi.mocked(safeInvoke).mockRejectedValue(new Error('not ready'));

    const result = await waitForDockerReady({
      maxAttempts: 2,
      intervalMs: 10,
      sleep: async () => {},
    });

    expect(result).toBe(false);
    expect(safeInvoke).toHaveBeenCalledTimes(2);
  });

  it('waitForDockerReady returns false when aborted', async () => {
    const controller = new AbortController();
    controller.abort();

    const result = await waitForDockerReady({
      signal: controller.signal,
      sleep: async () => {},
    });

    expect(result).toBe(false);
    expect(safeInvoke).not.toHaveBeenCalled();
  });
});

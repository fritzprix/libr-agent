import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Logger } from './logger-core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('Logger message length', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
    Logger.resetConfig();
  });

  afterEach(() => {
    Logger.resetConfig();
    vi.useRealTimers();
  });

  it('sends the full message with the limit, so the backend cuts it after masking', async () => {
    Logger.updateConfig({ maxMessageLength: 20 });
    const message = `prefix ${'v'.repeat(40)} suffix`;

    await Logger.info(message);
    await vi.advanceTimersByTimeAsync(600);

    expect(invoke).toHaveBeenCalledWith('log_batch', {
      entries: [
        expect.objectContaining({
          level: 'info',
          message: `[TauriAgent] ${message}`,
          maxMessageLength: 20,
        }),
      ],
    });
  });
});

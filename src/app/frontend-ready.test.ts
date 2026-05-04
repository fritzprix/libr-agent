import { beforeEach, describe, expect, it, vi } from 'vitest';
import { emit } from '@tauri-apps/api/event';

import {
  __resetFrontendReadyForTests,
  emitFrontendReadyOnce,
} from './frontend-ready';

const { loggerError } = vi.hoisted(() => ({
  loggerError: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  emit: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: loggerError,
  }),
}));

describe('frontend-ready', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetFrontendReadyForTests();
  });

  it('dedupes concurrent and repeated frontend-ready emits', async () => {
    let resolveEmit: (() => void) | undefined;

    vi.mocked(emit).mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveEmit = resolve;
        }),
    );

    const firstEmit = emitFrontendReadyOnce();
    const secondEmit = emitFrontendReadyOnce();

    expect(emit).toHaveBeenCalledTimes(1);

    resolveEmit?.();
    await Promise.all([firstEmit, secondEmit]);

    await emitFrontendReadyOnce();

    expect(emit).toHaveBeenCalledTimes(1);
  });

  it('retries frontend-ready after an emit failure', async () => {
    vi.mocked(emit)
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce(undefined);

    await emitFrontendReadyOnce();
    await emitFrontendReadyOnce();

    expect(emit).toHaveBeenCalledTimes(2);
    expect(loggerError).toHaveBeenCalledWith(
      'Failed to emit frontend-ready event',
      expect.any(Error),
    );
  });
});

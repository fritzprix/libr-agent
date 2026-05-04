import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { safeInvoke } from '@/lib/backend/core';
import {
  SettingsProvider,
  useSettings,
} from '../SettingsContext';
import { __resetRustSettingsServiceCacheForTests } from '@/lib/services/rust-settings-service';

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@/lib/performance/startup-metrics', () => ({
  markStartupMilestone: vi.fn(),
}));

describe('SettingsContext', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    __resetRustSettingsServiceCacheForTests();
  });

  it('dedupes the initial settings fetch across StrictMode remounts', async () => {
    vi.mocked(safeInvoke).mockResolvedValue([]);

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <React.StrictMode>
        <SettingsProvider>{children}</SettingsProvider>
      </React.StrictMode>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(safeInvoke).toHaveBeenCalledTimes(1);
    expect(safeInvoke).toHaveBeenCalledWith('list_settings');
  });
});

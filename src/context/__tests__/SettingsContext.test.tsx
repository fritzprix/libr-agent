import { renderHook, waitFor } from '@testing-library/react';
import { act } from '@testing-library/react';
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

  it('does not let an older settings fetch overwrite a refreshed cache', async () => {
    type SettingDto = {
      key: string;
      value: string;
      createdAt: number;
      updatedAt: number;
    };

    const createDeferred = <T,>() => {
      let resolve!: (value: T) => void;
      let reject!: (reason?: unknown) => void;
      const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      });

      return { promise, resolve, reject };
    };

    const staleSettings = createDeferred<SettingDto[]>();
    const freshSettings = createDeferred<SettingDto[]>();

    vi.mocked(safeInvoke).mockImplementation((command: string) => {
      if (command === 'list_settings') {
        const listCalls = vi
          .mocked(safeInvoke)
          .mock.calls.filter(([cmd]) => cmd === 'list_settings').length;
        return listCalls === 1 ? staleSettings.promise : freshSettings.promise;
      }

      if (command === 'update_settings') {
        return Promise.resolve([]);
      }

      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );

    const { result, unmount } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('list_settings');
    });

    act(() => {
      void result.current.update({ uiLanguage: 'ko' });
    });

    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('update_settings', {
        settings: { uiLanguage: 'ko' },
      });
      expect(vi.mocked(safeInvoke)).toHaveBeenCalledTimes(3);
    });

    freshSettings.resolve([
      { key: 'uiLanguage', value: 'ko', createdAt: 0, updatedAt: 0 },
    ]);

    await waitFor(() => {
      expect(result.current.value.uiLanguage).toBe('ko');
      expect(result.current.isLoading).toBe(false);
    });

    staleSettings.resolve([
      { key: 'uiLanguage', value: 'en', createdAt: 0, updatedAt: 0 },
    ]);
    await Promise.resolve();

    expect(result.current.value.uiLanguage).toBe('ko');

    unmount();

    const cachedRender = renderHook(() => useSettings(), { wrapper });
    await waitFor(() => {
      expect(cachedRender.result.current.value.uiLanguage).toBe('ko');
      expect(cachedRender.result.current.isLoading).toBe(false);
    });

    expect(vi.mocked(safeInvoke)).toHaveBeenCalledTimes(3);
  });
});

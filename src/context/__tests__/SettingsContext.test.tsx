import { render, renderHook, screen, waitFor } from '@testing-library/react';
import { act } from '@testing-library/react';
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { safeInvoke } from '@/lib/backend/core';
import {
  DEFAULT_SETTING,
  SettingsProvider,
  useSettings,
} from '../SettingsContext';
import { __resetRustSettingsServiceCacheForTests } from '@/lib/services/rust-settings-service';
import { settingsService } from '@/lib/services/settings-service';
import {
  TEST_ADVANCED_SETTINGS,
  TEST_DISPLAY_SETTINGS,
  TEST_EXPERIMENTAL_SETTINGS,
} from '@/test/fixtures/settings';

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

const i18nMock = vi.hoisted(() => ({
  language: 'ko',
  changeLanguage: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/lib/i18n', () => ({
  default: i18nMock,
}));

describe('SettingsContext', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    i18nMock.language = 'ko';
    __resetRustSettingsServiceCacheForTests();
  });

  it('dedupes the initial settings fetch across StrictMode remounts', async () => {
    vi.mocked(safeInvoke).mockImplementation((command: string) => {
      if (command === 'list_settings') {
        return Promise.resolve([]);
      }
      if (command === 'get_display_settings') {
        return Promise.resolve({ ...TEST_DISPLAY_SETTINGS });
      }
      if (command === 'get_experimental_settings') {
        return Promise.resolve({ ...TEST_EXPERIMENTAL_SETTINGS });
      }
      if (command === 'get_advanced_settings') {
        return Promise.resolve({ ...TEST_ADVANCED_SETTINGS });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <React.StrictMode>
        <SettingsProvider>{children}</SettingsProvider>
      </React.StrictMode>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(safeInvoke).toHaveBeenCalledTimes(4);
    expect(safeInvoke).toHaveBeenCalledWith('list_settings');
    expect(safeInvoke).toHaveBeenCalledWith('get_advanced_settings');
    expect(safeInvoke).toHaveBeenCalledWith('get_display_settings');
    expect(safeInvoke).toHaveBeenCalledWith('get_experimental_settings');
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

      if (command === 'get_display_settings') {
        return Promise.resolve({ ...TEST_DISPLAY_SETTINGS });
      }
      if (command === 'get_experimental_settings') {
        return Promise.resolve({ ...TEST_EXPERIMENTAL_SETTINGS });
      }
      if (command === 'get_advanced_settings') {
        return Promise.resolve({ ...TEST_ADVANCED_SETTINGS });
      }

      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    // The provider renders no consumer before the first load, so drive the
    // service directly: an update lands while the initial fetch is in flight.
    const initial = settingsService.getSettings();
    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('list_settings');
    });

    const updating = settingsService.updateSettings({ uiLanguage: 'ko' });
    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('update_settings', {
        settings: { uiLanguage: 'ko' },
      });
    });

    freshSettings.resolve([
      { key: 'uiLanguage', value: 'ko', createdAt: 0, updatedAt: 0 },
    ]);
    expect((await updating).uiLanguage).toBe('ko');

    staleSettings.resolve([
      { key: 'uiLanguage', value: 'en', createdAt: 0, updatedAt: 0 },
    ]);
    await initial;

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );
    const cachedRender = renderHook(() => useSettings(), { wrapper });
    await waitFor(() => {
      expect(cachedRender.result.current.value.uiLanguage).toBe('ko');
      expect(cachedRender.result.current.isLoading).toBe(false);
    });
  });

  it('syncs i18n language from persisted settings after initial load', async () => {
    expect(i18nMock.language).toBe('ko');

    vi.mocked(safeInvoke).mockResolvedValue([
      { key: 'uiLanguage', value: 'en', createdAt: 0, updatedAt: 0 },
    ]);

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(i18nMock.changeLanguage).toHaveBeenCalledWith('en');
  });

  it('does not call changeLanguage when persisted language matches i18n', async () => {
    i18nMock.language = 'en';

    vi.mocked(safeInvoke).mockResolvedValue([
      { key: 'uiLanguage', value: 'en', createdAt: 0, updatedAt: 0 },
    ]);

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(i18nMock.changeLanguage).not.toHaveBeenCalled();
  });

  it('falls back to DEFAULT_SETTING.uiLanguage when uiLanguage is missing', async () => {
    expect(i18nMock.language).toBe('ko');

    vi.mocked(safeInvoke).mockResolvedValue([]);

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.value.uiLanguage).toBe(DEFAULT_SETTING.uiLanguage);
    expect(i18nMock.changeLanguage).toHaveBeenCalledWith(
      DEFAULT_SETTING.uiLanguage,
    );
  });

  it('renders children only after the first load', async () => {
    let resolveAdvanced: (value: unknown) => void = () => {};
    vi.mocked(safeInvoke).mockImplementation((command: string) => {
      if (command === 'list_settings') {
        return Promise.resolve([]);
      }
      if (command === 'get_display_settings') {
        return Promise.resolve({ ...TEST_DISPLAY_SETTINGS });
      }
      if (command === 'get_experimental_settings') {
        return Promise.resolve({ ...TEST_EXPERIMENTAL_SETTINGS });
      }
      if (command === 'get_advanced_settings') {
        return new Promise((resolve) => {
          resolveAdvanced = resolve;
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    render(
      <SettingsProvider>
        <span>app</span>
      </SettingsProvider>,
    );
    expect(screen.queryByText('app')).toBeNull();

    await act(async () => {
      resolveAdvanced({ ...TEST_ADVANCED_SETTINGS });
    });
    expect(await screen.findByText('app')).toBeTruthy();
  });

  it('shows the error instead of children when the first load fails', async () => {
    vi.mocked(safeInvoke).mockRejectedValue(new Error('db unreadable'));

    render(
      <SettingsProvider>
        <span>app</span>
      </SettingsProvider>,
    );

    expect((await screen.findByRole('alert')).textContent).toContain(
      'db unreadable',
    );
    expect(screen.queryByText('app')).toBeNull();
  });

  it('keeps the last loaded settings when a reload fails', async () => {
    vi.mocked(safeInvoke).mockImplementation((command: string) => {
      if (command === 'list_settings') {
        return Promise.resolve([]);
      }
      if (command === 'get_display_settings') {
        return Promise.resolve({ ...TEST_DISPLAY_SETTINGS });
      }
      if (command === 'get_experimental_settings') {
        return Promise.resolve({ ...TEST_EXPERIMENTAL_SETTINGS });
      }
      if (command === 'get_advanced_settings') {
        return Promise.resolve({ ...TEST_ADVANCED_SETTINGS, maxRetries: 7 });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsProvider>{children}</SettingsProvider>
    );
    const { result } = renderHook(() => useSettings(), { wrapper });
    await waitFor(() => expect(result.current.value.advanced.maxRetries).toBe(7));

    vi.mocked(safeInvoke).mockRejectedValue(new Error('db unreadable'));
    await act(async () => {
      await expect(result.current.update({ uiLanguage: 'en' })).rejects.toThrow();
    });

    expect(result.current.value.advanced.maxRetries).toBe(7);
  });
});

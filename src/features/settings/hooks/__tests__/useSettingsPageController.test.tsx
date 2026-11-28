import type { ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';
import { TestSettingsProvider } from '@/test/fixtures/TestSettingsProvider';
import { useSettingsPageController } from '../useSettingsPageController';

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('swr', () => ({
  mutate: vi.fn().mockResolvedValue(undefined),
}));

function wrapper({ children }: { children: ReactNode }) {
  return (
    <MemoryRouter>
      <TestSettingsProvider>{children}</TestSettingsProvider>
    </MemoryRouter>
  );
}

describe('useSettingsPageController handleSave toast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows only the success toast when no restart is required', async () => {
    const { result } = renderHook(() => useSettingsPageController(), {
      wrapper,
    });

    act(() => {
      result.current.updateAdvanced('diffContextLines', 99);
    });

    let saved = false;
    await act(async () => {
      saved = await result.current.handleSave();
    });

    expect(saved).toBe(true);
    expect(toast.success).toHaveBeenCalledTimes(1);
    expect(toast.success).toHaveBeenCalledWith('Settings saved successfully');
    expect(toast.info).not.toHaveBeenCalled();
    expect(toast.warning).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it('shows one saved toast with the restart action when a restart is required', async () => {
    const { result } = renderHook(() => useSettingsPageController(), {
      wrapper,
    });

    act(() => {
      result.current.updateAdvanced('maxConcurrentActiveSessions', 99);
    });
    expect(result.current.restartRequiredSettingsChanged).toBe(true);

    let saved = false;
    await act(async () => {
      saved = await result.current.handleSave();
    });

    expect(saved).toBe(true);
    expect(toast.success).toHaveBeenCalledTimes(1);
    expect(toast.success).toHaveBeenCalledWith(
      'Settings saved. Some settings are applied after restarting the app.',
      expect.objectContaining({
        action: expect.objectContaining({
          label: 'Restart now',
          onClick: expect.any(Function),
        }),
      }),
    );
    expect(toast.info).not.toHaveBeenCalled();
    expect(toast.warning).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
  });
});

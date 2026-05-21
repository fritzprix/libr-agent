import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useSettingsForm } from '../useSettingsForm';
import { DEFAULT_SETTING } from '@/context/SettingsContext';
import { AIServiceProvider } from '@/lib/ai-service';

// Mock dependencies
const mockUpdateGlobal = vi.fn();
let currentGlobalSettings = { ...DEFAULT_SETTING };

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: currentGlobalSettings,
    update: mockUpdateGlobal,
  }),
}));

function cloneSettings<T>(settings: T): T {
  return structuredClone(settings);
}

describe('useSettingsForm', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentGlobalSettings = { ...DEFAULT_SETTING };
  });

  it('should initialize with global settings', () => {
    const { result } = renderHook(() => useSettingsForm());
    expect(result.current.formState).toEqual(DEFAULT_SETTING);
    expect(result.current.isDirty).toBe(false);
    expect(result.current.dirtyState).toEqual({
      general: false,
      'ai-models': false,
      'chat-interface': false,
      system: false,
      advanced: false,
      dev: false,
      experimental: false,
    });
  });

  it('should update top-level settings', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(true);
    expect(result.current.dirtyState['chat-interface']).toBe(true);
    expect(result.current.dirtyState.general).toBe(false);
  });

  it('should update service config', () => {
    const { result } = renderHook(() => useSettingsForm());
    const provider = AIServiceProvider.OpenAI;

    act(() => {
      result.current.updateServiceConfig(provider, { apiKey: 'test-key' });
    });

    expect(result.current.formState.serviceConfigs[provider].apiKey).toBe('test-key');
    expect(result.current.isDirty).toBe(true);
  });

  it('should update advanced settings', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.updateAdvanced('maxRetries', 5);
    });

    expect(result.current.formState.advanced.maxRetries).toBe(5);
    expect(result.current.isDirty).toBe(true);
    expect(result.current.dirtyState['ai-models']).toBe(true);
    expect(result.current.dirtyState.advanced).toBe(false);
  });

  it('should reset changes', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });
    expect(result.current.isDirty).toBe(true);

    act(() => {
      result.current.reset();
    });

    expect(result.current.formState).toEqual(DEFAULT_SETTING);
    expect(result.current.isDirty).toBe(false);
  });

  it('should save changes', async () => {
    mockUpdateGlobal.mockImplementation(async (nextSettings) => {
      currentGlobalSettings = cloneSettings(nextSettings);
    });

    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    await act(async () => {
      await result.current.save();
    });

    act(() => {
      rerender();
    });

    expect(mockUpdateGlobal).toHaveBeenCalledWith(
      expect.objectContaining({
        windowSize: 50,
      }),
    );
    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(false);
  });

  it('should keep the draft visible until saved globals catch up', async () => {
    mockUpdateGlobal.mockResolvedValue(undefined);

    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    await act(async () => {
      await result.current.save();
    });

    expect(mockUpdateGlobal).toHaveBeenCalledWith(
      expect.objectContaining({
        windowSize: 50,
      }),
    );
    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(true);

    act(() => {
      currentGlobalSettings = {
        ...cloneSettings(DEFAULT_SETTING),
        windowSize: 50,
      };
      rerender();
    });

    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(false);
  });

  it('should preserve dirtyState reference across rerenders when inputs are unchanged', () => {
    const { result, rerender } = renderHook(() => useSettingsForm());

    const initialDirtyState = result.current.dirtyState;

    act(() => {
      rerender();
    });

    expect(result.current.dirtyState).toBe(initialDirtyState);
  });

  it('should update display settings', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.updateDisplay('metricDisplayMode', 'tooltip');
    });

    expect(result.current.formState.display.metricDisplayMode).toBe('tooltip');
    expect(result.current.isDirty).toBe(true);
  });

  it('should update system settings', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.updateSystem('maxFileUploadSizeMB', 100);
    });

    expect(result.current.formState.system.maxFileUploadSizeMB).toBe(100);
    expect(result.current.isDirty).toBe(true);
  });

  it('should adopt external settings changes while pristine', () => {
    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      currentGlobalSettings = {
        ...cloneSettings(DEFAULT_SETTING),
        windowSize: 64,
      };
      rerender();
    });

    expect(result.current.formState.windowSize).toBe(64);
    expect(result.current.isDirty).toBe(false);
  });

  it('should preserve local edits when global settings change externally', () => {
    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    act(() => {
      currentGlobalSettings = {
        ...cloneSettings(DEFAULT_SETTING),
        windowSize: 64,
      };
      rerender();
    });

    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(true);
    expect(result.current.dirtyState['chat-interface']).toBe(true);
  });

  it('should compare subsequent edits against the latest global settings', () => {
    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    act(() => {
      currentGlobalSettings = {
        ...cloneSettings(DEFAULT_SETTING),
        windowSize: 64,
      };
      rerender();
    });

    act(() => {
      result.current.update('windowSize', 64);
    });

    expect(result.current.formState.windowSize).toBe(64);
    expect(result.current.isDirty).toBe(false);
    expect(result.current.dirtyState['chat-interface']).toBe(false);
  });

  it('should reset to the latest global settings after an external change', () => {
    const { result, rerender } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    act(() => {
      currentGlobalSettings = {
        ...cloneSettings(DEFAULT_SETTING),
        windowSize: 64,
      };
      rerender();
    });

    act(() => {
      result.current.reset();
    });

    expect(result.current.formState.windowSize).toBe(64);
    expect(result.current.isDirty).toBe(false);
    expect(result.current.dirtyState['chat-interface']).toBe(false);
  });

  it('should update experimental settings and track dirty state correctly', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.updateExperimental('inlineAudioAttachment', false);
    });

    expect(result.current.formState.experimental.inlineAudioAttachment).toBe(false);
    expect(result.current.isDirty).toBe(true);
    expect(result.current.dirtyState.experimental).toBe(true);
  });
});


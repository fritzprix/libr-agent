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

describe('useSettingsForm', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentGlobalSettings = { ...DEFAULT_SETTING };
  });

  it('should initialize with global settings', () => {
    const { result } = renderHook(() => useSettingsForm());
    expect(result.current.formState).toEqual(DEFAULT_SETTING);
    expect(result.current.isDirty).toBe(false);
  });

  it('should update top-level settings', () => {
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    expect(result.current.formState.windowSize).toBe(50);
    expect(result.current.isDirty).toBe(true);
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
    const { result } = renderHook(() => useSettingsForm());

    act(() => {
      result.current.update('windowSize', 50);
    });

    await act(async () => {
      await result.current.save();
    });

    expect(mockUpdateGlobal).toHaveBeenCalledWith(expect.objectContaining({
      windowSize: 50
    }));
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
});

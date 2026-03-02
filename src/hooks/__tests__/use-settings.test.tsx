import { renderHook } from '@testing-library/react';
import { useSettings } from '../use-settings';
import { SettingsContext } from '@/context/SettingsContext';
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import React from 'react';
import { DEFAULT_SETTING } from '@/lib/services/settings-service';

// Mock console.error to prevent act warnings from cluttering output during the expected error test
const originalError = console.error;
beforeAll(() => {
  console.error = (...args) => {
    if (typeof args[0] === 'string' && args[0].includes('The above error occurred in the <TestComponent> component:')) {
      return;
    }
    originalError.call(console, ...args);
  };
});
afterAll(() => {
  console.error = originalError;
});

describe('useSettings', () => {
  it('throws an error if used outside of SettingsProvider', () => {
    expect(() => renderHook(() => useSettings())).toThrowError(
      'useSettings must be used within a SettingsProvider',
    );
  });

  it('returns context value when used within SettingsProvider', () => {
    // We cannot use SettingsContextType as an import because it is not exported,
    // so we just define a valid object literal matching its expected shape:
    const mockContextValue = {
      value: DEFAULT_SETTING,
      update: async () => { },
      isLoading: false,
      error: null,
    };

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsContext.Provider value={mockContextValue as unknown as React.ContextType<typeof SettingsContext>}>
        {children}
      </SettingsContext.Provider>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    expect(result.current).toBe(mockContextValue);
    expect(result.current.value.preferredModel.provider).toBe(DEFAULT_SETTING.preferredModel.provider);
  });
});

import { renderHook } from '@testing-library/react';
import { useSettings } from '../use-settings';
import { SettingsContext, SettingsContextType } from '@/context/SettingsContext';
import { describe, it, expect, vi, beforeAll, afterAll } from 'vitest';
import React from 'react';

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
    // Suppress console.error for expected thrown error
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    expect(() => renderHook(() => useSettings())).toThrowError(
      'useSettings must be used within a SettingsProvider',
    );

    consoleSpy.mockRestore();
  });

  it('returns context value when used within SettingsProvider', () => {
    const mockContextValue: SettingsContextType = {
      settings: {
        apiKeys: {},
        llmProvider: 'openai',
        defaultModel: 'gpt-4o',
        theme: 'system',
        developerMode: false,
        mcp: {
          servers: {}
        }
      },
      updateSettings: vi.fn(),
      isLoaded: true,
      error: null,
    };

    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <SettingsContext.Provider value={mockContextValue}>
        {children}
      </SettingsContext.Provider>
    );

    const { result } = renderHook(() => useSettings(), { wrapper });

    expect(result.current).toBe(mockContextValue);
    expect(result.current.settings.llmProvider).toBe('openai');
  });
});

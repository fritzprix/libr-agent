import { useMemo, type ReactNode } from 'react';
import { SettingsContext, type Settings } from '@/context/SettingsContext';
import { TEST_SETTINGS } from './settings';

/**
 * Loaded settings for tests of providers that consume settings but do not
 * test settings loading (the real `SettingsProvider` renders nothing until
 * the backend answers).
 */
export function TestSettingsProvider({
  children,
  value = TEST_SETTINGS,
}: {
  children: ReactNode;
  value?: Settings;
}) {
  const context = useMemo(
    () => ({
      value,
      update: async () => {},
      isLoading: false,
      error: null,
    }),
    [value],
  );
  return (
    <SettingsContext.Provider value={context}>
      {children}
    </SettingsContext.Provider>
  );
}

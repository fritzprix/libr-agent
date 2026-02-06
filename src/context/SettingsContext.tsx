import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { getLogger } from '../lib/logger';
import {
  settingsService,
  Settings,
  ServiceConfig,
  AdvancedSettings,
  DisplaySettings,
  SystemSettings,
  DEFAULT_SETTING,
} from '@/lib/services/settings-service';

const logger = getLogger('SettingsContext');

// Re-export types for backward compatibility
export type {
  Settings,
  ServiceConfig,
  AdvancedSettings,
  DisplaySettings,
  SystemSettings,
};
export { DEFAULT_SETTING };

export interface SettingsContextType {
  value: Settings;
  update: (settings: Partial<Settings>) => Promise<void>;
  isLoading: boolean;
  error: Error | null;
}

interface SettingModalViewContextType {
  isOpen: boolean;
  toggleOpen: () => void;
}

export const SettingModalViewContext =
  createContext<SettingModalViewContextType>({
    isOpen: false,
    toggleOpen: () => {},
  });

export const SettingsContext = createContext<SettingsContextType | undefined>(
  undefined,
);

export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const [openSettingModal, setOpenSettingModal] = useState(false);

  // Use the singleton service instance
  // const settingsService = useMemo(() => new LocalSettingsService(), []);
  // We can use the imported singleton directly, but to keep the variable name consistent in scope:
  // const svc = settingsService;
  // Actually, the useAsyncFn below uses `settingsService` variable.
  // I will just use the imported one directly in the hook dependency array.

  const [{ value, loading, error }, load] = useAsyncFn(async () => {
    const settings = await settingsService.getSettings();
    return settings;
  }, [settingsService]);

  useEffect(() => {
    load();
  }, [load]);

  // Update method
  const update = useCallback(
    async (settings: Partial<Settings>) => {
      try {
        await settingsService.updateSettings(settings);
        await load();
      } catch (e) {
        logger.error('Failed to update settings', e);
        throw e;
      }
    },
    [load, settingsService],
  );

  const contextValue: SettingsContextType = useMemo(() => {
    const finalValue = value || DEFAULT_SETTING;

    return {
      value: finalValue,
      isLoading: loading,
      update,
      error: error ?? null,
    };
  }, [value, loading, update, error]);

  const modalViewContextValue: SettingModalViewContextType = useMemo(() => {
    return {
      isOpen: openSettingModal,
      toggleOpen: () => setOpenSettingModal((prev) => !prev),
    };
  }, [openSettingModal]);

  return (
    <SettingModalViewContext.Provider value={modalViewContextValue}>
      <SettingsContext.Provider value={contextValue}>
        {children}
      </SettingsContext.Provider>
    </SettingModalViewContext.Provider>
  );
}

export const useSettings = () => {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error('useSettings must be used within a SettingsProvider');
  }
  return context;
};

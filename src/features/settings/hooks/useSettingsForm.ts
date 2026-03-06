import { useState, useCallback, useMemo } from 'react';
import { useSettings } from '@/hooks/use-settings';
import { AIServiceProvider } from '@/lib/ai-service';
import type {
  ServiceConfig,
  AdvancedSettings,
  DisplaySettings,
  SystemSettings,
  Settings,
} from '@/context/SettingsContext';
import equal from 'fast-deep-equal';

// Define the form state shape, identical to Settings for now
export type SettingsFormState = Settings;

export function useSettingsForm() {
  const { value: globalSettings, update: updateGlobal } = useSettings();

  // Initialize form state from global settings
  const [formState, setFormState] = useState<SettingsFormState>(globalSettings);
  const [prevGlobalSettings, setPrevGlobalSettings] = useState(globalSettings);

  // Sync form state if globalSettings changes externally
  if (globalSettings !== prevGlobalSettings) {
    setPrevGlobalSettings(globalSettings);
    setFormState(globalSettings);
  }

  // Generic update for top-level keys
  const update = useCallback(
    <K extends keyof SettingsFormState>(
      key: K,
      value: SettingsFormState[K],
    ) => {
      setFormState((prev) => ({
        ...prev,
        [key]: value,
      }));
    },
    [],
  );

  // Specialized updaters for nested objects to keep usage clean
  const updateServiceConfig = useCallback(
    (provider: AIServiceProvider, patch: Partial<ServiceConfig>) => {
      setFormState((prev) => {
        const currentConfig = prev.serviceConfigs[provider] || {};
        const newConfig = { ...currentConfig, ...patch };
        return {
          ...prev,
          serviceConfigs: {
            ...prev.serviceConfigs,
            [provider]: newConfig,
          },
        };
      });
    },
    [],
  );

  const updateAdvanced = useCallback(
    <K extends keyof AdvancedSettings>(key: K, value: AdvancedSettings[K]) => {
      setFormState((prev) => ({
        ...prev,
        advanced: {
          ...prev.advanced,
          [key]: value,
        },
      }));
    },
    [],
  );

  const updateDisplay = useCallback(
    <K extends keyof DisplaySettings>(key: K, value: DisplaySettings[K]) => {
      setFormState((prev) => ({
        ...prev,
        display: {
          ...prev.display,
          [key]: value,
        },
      }));
    },
    [],
  );

  const updateSystem = useCallback(
    <K extends keyof SystemSettings>(key: K, value: SystemSettings[K]) => {
      setFormState((prev) => ({
        ...prev,
        system: {
          ...prev.system,
          [key]: value,
        },
      }));
    },
    [],
  );

  const reset = useCallback(() => {
    setFormState(globalSettings);
  }, [globalSettings]);

  const save = useCallback(async () => {
    await updateGlobal(formState);
  }, [formState, updateGlobal]);

  const isDirty = useMemo(() => {
    return !equal(formState, globalSettings);
  }, [formState, globalSettings]);

  return {
    formState,
    update,
    updateServiceConfig,
    updateAdvanced,
    updateDisplay,
    updateSystem,
    reset,
    save,
    isDirty,
  };
}

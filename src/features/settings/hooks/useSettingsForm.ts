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

export type SettingsFormState = Settings;

export type SettingsDirtyState = {
  general: boolean;
  'ai-models': boolean;
  'chat-interface': boolean;
  system: boolean;
  advanced: boolean;
  dev: boolean;
};

function getEmptyDirtyState(): SettingsDirtyState {
  return {
    general: false,
    'ai-models': false,
    'chat-interface': false,
    system: false,
    advanced: false,
    dev: false,
  };
}

function getAiModelsComparableState(settings: SettingsFormState) {
  return {
    serviceConfigs: settings.serviceConfigs,
    preferredModel: settings.preferredModel,
    fallbackModel: settings.fallbackModel,
    agentHubUrl: settings.agentHubUrl,
    maxRetries: settings.advanced.maxRetries,
    retryDelay: settings.advanced.retryDelay,
    defaultMaxOutputTokens: settings.advanced.defaultMaxOutputTokens,
  };
}

function getChatInterfaceComparableState(settings: SettingsFormState) {
  return {
    contextStrategy: settings.contextStrategy,
    windowSize: settings.windowSize,
    maxInputContext: settings.maxInputContext,
    toolCallGroupVisibleCount: settings.toolCallGroupVisibleCount,
    diffContextLines: settings.advanced.diffContextLines,
  };
}

function getSystemComparableState(settings: SettingsFormState) {
  return {
    skillsDirectory: settings.system.skillsDirectory,
    maxFileUploadSizeMB: settings.system.maxFileUploadSizeMB,
    searchIndexFrequencyMinutes: settings.system.searchIndexFrequencyMinutes,
    webActionTimeoutSeconds: settings.system.webActionTimeoutSeconds,
    mcpServerStartupTimeoutSeconds:
      settings.system.mcpServerStartupTimeoutSeconds,
    mcpToolTimeoutSeconds: settings.system.mcpToolTimeoutSeconds,
    scheduledTaskMinimumIntervalMinutes:
      settings.system.scheduledTaskMinimumIntervalMinutes,
    maxScheduledTaskGroups: settings.system.maxScheduledTaskGroups,
    httpServerPort: settings.system.httpServerPort,
    httpServerExpose: settings.system.httpServerExpose,
  };
}

function getAdvancedComparableState(settings: SettingsFormState) {
  const { diffContextLines, ...advancedWithoutChatFields } = settings.advanced;
  void diffContextLines;

  return {
    ...advancedWithoutChatFields,
    maxRetries: undefined,
    retryDelay: undefined,
    defaultMaxOutputTokens: undefined,
    shellIsolationLevel: settings.system.shellIsolationLevel,
  };
}

export function getSettingsDirtyState(
  formState: SettingsFormState,
  globalSettings: SettingsFormState,
): SettingsDirtyState {
  return {
    general:
      formState.uiLanguage !== globalSettings.uiLanguage ||
      !equal(formState.display, globalSettings.display),
    'ai-models': !equal(
      getAiModelsComparableState(formState),
      getAiModelsComparableState(globalSettings),
    ),
    'chat-interface': !equal(
      getChatInterfaceComparableState(formState),
      getChatInterfaceComparableState(globalSettings),
    ),
    system: !equal(
      getSystemComparableState(formState),
      getSystemComparableState(globalSettings),
    ),
    advanced: !equal(
      getAdvancedComparableState(formState),
      getAdvancedComparableState(globalSettings),
    ),
    dev: false,
  };
}

export function useSettingsForm() {
  const { value: globalSettings, update: updateGlobal } = useSettings();

  const [draftState, setDraftState] = useState<SettingsFormState | null>(null);
  const [prevGlobalSettings, setPrevGlobalSettings] = useState<SettingsFormState>(globalSettings);

  if (globalSettings !== prevGlobalSettings) {
    setPrevGlobalSettings(globalSettings);
    // If we're dirty but globals changed from underneath us, reset the form if it exactly matches the new globals.
    if (draftState && equal(draftState, globalSettings)) {
      setDraftState(null);
    }
  }

  const activeFormState = draftState ? draftState : globalSettings;
  const activeDirtyState = useMemo(() => {
    return draftState
      ? getSettingsDirtyState(draftState, globalSettings)
      : getEmptyDirtyState();
  }, [draftState, globalSettings]);

  const isDirty = useMemo(() => {
    return Object.values(activeDirtyState).some(Boolean);
  }, [activeDirtyState]);

  const updateFormStore = useCallback(
    (updater: (previous: SettingsFormState) => SettingsFormState) => {
      setDraftState((prevDraft) => {
        const previous = prevDraft ? prevDraft : globalSettings;
        const newState = updater(previous);
        return equal(newState, globalSettings) ? null : newState;
      });
    },
    [globalSettings],
  );

  const update = useCallback(
    <K extends keyof SettingsFormState>(
      key: K,
      value: SettingsFormState[K],
    ) => {
      updateFormStore((prev) => ({
        ...prev,
        [key]: value,
      }));
    },
    [updateFormStore],
  );

  const updateServiceConfig = useCallback(
    (provider: AIServiceProvider, patch: Partial<ServiceConfig>) => {
      updateFormStore((prev) => {
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
    [updateFormStore],
  );

  const updateAdvanced = useCallback(
    <K extends keyof AdvancedSettings>(key: K, value: AdvancedSettings[K]) => {
      updateFormStore((prev) => ({
        ...prev,
        advanced: {
          ...prev.advanced,
          [key]: value,
        },
      }));
    },
    [updateFormStore],
  );

  const updateDisplay = useCallback(
    <K extends keyof DisplaySettings>(key: K, value: DisplaySettings[K]) => {
      updateFormStore((prev) => ({
        ...prev,
        display: {
          ...prev.display,
          [key]: value,
        },
      }));
    },
    [updateFormStore],
  );

  const updateSystem = useCallback(
    <K extends keyof SystemSettings>(key: K, value: SystemSettings[K]) => {
      updateFormStore((prev) => ({
        ...prev,
        system: {
          ...prev.system,
          [key]: value,
        },
      }));
    },
    [updateFormStore],
  );

  const reset = useCallback(() => {
    setDraftState(null);
  }, []);

  const save = useCallback(async () => {
    if (!draftState) return;
    await updateGlobal(draftState);
  }, [draftState, updateGlobal]);

  return {
    formState: activeFormState,
    dirtyState: activeDirtyState,
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

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { AIServiceProvider } from '../lib/ai-service';
import { dbService } from '../lib/db';
import { llmConfigManager } from '../lib/llm-config-manager';
import { getLogger } from '../lib/logger';
import SettingsModal from '@/features/settings/SettingsModal';

const logger = getLogger('SettingsContext');

interface ModelChoice {
  provider: AIServiceProvider;
  model: string;
}

export interface ServiceConfig {
  apiKey?: string;
  baseUrl?: string;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  preferredModel: ModelChoice;
  windowSize: number;
}

const DEFAULT_MODEL = llmConfigManager.recommendModel({});

export const DEFAULT_SETTING: Settings = {
  serviceConfigs: Object.values(AIServiceProvider).reduce(
    (acc, provider) => {
      acc[provider] = {};
      return acc;
    },
    {} as Record<AIServiceProvider, ServiceConfig>,
  ),
  preferredModel: {
    provider: (DEFAULT_MODEL?.providerId || 'openai') as AIServiceProvider,
    model: DEFAULT_MODEL?.modelId || '',
  },
  windowSize: 20,
};

interface SettingsContextType {
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
  const [{ value, loading, error }, load] = useAsyncFn(async () => {
    try {
      const [
        serviceConfigsObject,
        apiKeysObject,
        preferredModelObject,
        windowSizeObject,
      ] = await Promise.all([
        dbService.objects.read('serviceConfigs'),
        dbService.objects.read('apiKeys'), // for backward compatibility
        dbService.objects.read('preferredModel'),
        dbService.objects.read('windowSize'),
      ]);

      // Handle migration from old format to new format
      let serviceConfigs: Record<AIServiceProvider, ServiceConfig> =
        DEFAULT_SETTING.serviceConfigs;
      if (serviceConfigsObject) {
        serviceConfigs = {
          ...DEFAULT_SETTING.serviceConfigs,
          ...(serviceConfigsObject.value as Record<
            AIServiceProvider,
            ServiceConfig
          >),
        };
      } else if (apiKeysObject) {
        // Migrate old format to new format
        const oldApiKeys = apiKeysObject.value as Record<
          AIServiceProvider,
          string
        >;
        serviceConfigs = Object.entries(oldApiKeys).reduce(
          (acc, [provider, apiKey]) => {
            acc[provider as AIServiceProvider] = { apiKey };
            return acc;
          },
          { ...DEFAULT_SETTING.serviceConfigs },
        );
        // Save migrated data
        await dbService.objects.upsert({
          key: 'serviceConfigs',
          value: serviceConfigs,
        });
      }

      const settings: Settings = {
        ...DEFAULT_SETTING,
        serviceConfigs,
        ...(preferredModelObject
          ? { preferredModel: preferredModelObject.value as ModelChoice }
          : {}),
        ...(windowSizeObject != null
          ? { windowSize: windowSizeObject.value as number }
          : {}),
      };
      return settings;
    } catch (e) {
      logger.error('Failed to load settings', e);
      throw e;
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // Update method
  const update = useCallback(
    async (settings: Partial<Settings>) => {
      try {
        if (settings.serviceConfigs) {
          const newServiceConfigs = {
            ...(value?.serviceConfigs || {}),
            ...settings.serviceConfigs,
          };
          await dbService.objects.upsert({
            key: 'serviceConfigs',
            value: newServiceConfigs,
          });
        }
        if (settings.preferredModel) {
          await dbService.objects.upsert({
            key: 'preferredModel',
            value: settings.preferredModel,
          });
        }
        if (settings.windowSize != null) {
          await dbService.objects.upsert({
            key: 'windowSize',
            value: settings.windowSize,
          });
        }
        await load();
      } catch (e) {
        logger.error('Failed to update settings', e);
        throw e;
      }
    },
    [load, value],
  );

  const contextValue: SettingsContextType = useMemo(() => {
    return {
      value: value || DEFAULT_SETTING,
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
  }, []);

  return (
    <SettingModalViewContext.Provider value={modalViewContextValue}>
      <SettingsContext.Provider value={contextValue}>
        {children}
        <SettingsModal
          isOpen={openSettingModal}
          onClose={() => setOpenSettingModal((prev) => !prev)}
        />
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

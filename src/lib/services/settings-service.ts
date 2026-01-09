import { dbService } from '@/lib/db/service';
import { getLogger } from '@/lib/logger';
import { AIServiceProvider } from '@/lib/ai-service';
import { llmConfigManager } from '@/lib/llm-config-manager';

const logger = getLogger('SettingsService');

export interface SafetySetting {
  category: string;
  threshold: string;
}

export interface ServiceConfig {
  apiKey?: string;
  baseUrl?: string;
  safetySettings?: SafetySetting[];
}

export interface ModelChoice {
  provider: AIServiceProvider;
  model: string;
}

export interface AdvancedSettings {
  maxRetries: number;
  retryDelay: number; // in milliseconds
  circuitBreakerThreshold: number;
}

export interface DisplaySettings {
  metricDisplayMode: 'tooltip' | 'inline';
  prefillDisplayFormat: 'time' | 'tokensPerSecond';
  showTokenSpeed: boolean;
  compactMetrics: boolean;
}

export interface Settings {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  preferredModel: ModelChoice;
  windowSize: number;
  uiLanguage: string;
  toolCallGroupVisibleCount: number;
  agentHubUrl?: string;
  advanced: AdvancedSettings;
  display: DisplaySettings;
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
  uiLanguage: 'en',
  toolCallGroupVisibleCount: 4,
  agentHubUrl: '',
  advanced: {
    maxRetries: 1,
    retryDelay: 5000,
    circuitBreakerThreshold: 3,
  },
  display: {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
  },
};

export interface ISettingsService {
  getSettings(): Promise<Settings>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
}

export class LocalSettingsService implements ISettingsService {
  async getSettings(): Promise<Settings> {
    try {
      const [
        serviceConfigsObject,
        apiKeysObject,
        preferredModelObject,
        windowSizeObject,
        uiLanguageObject,
        toolCallGroupVisibleCountObject,
        agentHubUrlObject,
        advancedSettingsObject,
        displaySettingsObject,
      ] = await Promise.all([
        dbService.objects.read('serviceConfigs'),
        dbService.objects.read('apiKeys'), // for backward compatibility
        dbService.objects.read('preferredModel'),
        dbService.objects.read('windowSize'),
        dbService.objects.read('uiLanguage'),
        dbService.objects.read('toolCallGroupVisibleCount'),
        dbService.objects.read('agentHubUrl'),
        dbService.objects.read('advancedSettings'),
        dbService.objects.read('displaySettings'),
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
        ...(uiLanguageObject != null
          ? { uiLanguage: uiLanguageObject.value as string }
          : {}),
        ...(toolCallGroupVisibleCountObject != null
          ? {
              toolCallGroupVisibleCount:
                toolCallGroupVisibleCountObject.value as number,
            }
          : {}),
        ...(agentHubUrlObject != null
          ? { agentHubUrl: agentHubUrlObject.value as string }
          : {}),
        ...(advancedSettingsObject != null
          ? { advanced: advancedSettingsObject.value as AdvancedSettings }
          : {}),
        ...(displaySettingsObject != null
          ? { display: displaySettingsObject.value as DisplaySettings }
          : {}),
      };
      return settings;
    } catch (e) {
      logger.error('Failed to load settings', e);
      throw e;
    }
  }

  async updateSettings(settings: Partial<Settings>): Promise<Settings> {
    try {
      // We need to load current settings first to merge?
      // Or just update individual keys as in the original code.
      // The original code updates individual keys in DB, then reloads.
      // Here we can do the same.

      // Note: The original code accessed `value` (current state) to merge serviceConfigs.
      // We should probably fetch current settings if we need to merge, or assume the caller passes what they want.
      // However, for serviceConfigs, it does a merge:
      // const newServiceConfigs = { ...(value?.serviceConfigs || {}), ...settings.serviceConfigs };

      // To be safe and stateless, we should probably read current serviceConfigs from DB if we are updating it.

      if (settings.serviceConfigs) {
        const currentServiceConfigsObj =
          await dbService.objects.read('serviceConfigs');
        const currentServiceConfigs = currentServiceConfigsObj
          ? (currentServiceConfigsObj.value as Record<
              AIServiceProvider,
              ServiceConfig
            >)
          : DEFAULT_SETTING.serviceConfigs;

        const newServiceConfigs = {
          ...currentServiceConfigs,
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
      if (settings.uiLanguage != null) {
        await dbService.objects.upsert({
          key: 'uiLanguage',
          value: settings.uiLanguage,
        });
      }
      if (settings.toolCallGroupVisibleCount != null) {
        await dbService.objects.upsert({
          key: 'toolCallGroupVisibleCount',
          value: settings.toolCallGroupVisibleCount,
        });
      }
      if (settings.agentHubUrl != null) {
        await dbService.objects.upsert({
          key: 'agentHubUrl',
          value: settings.agentHubUrl,
        });
      }
      if (settings.advanced) {
        await dbService.objects.upsert({
          key: 'advancedSettings',
          value: settings.advanced,
        });
      }
      if (settings.display) {
        await dbService.objects.upsert({
          key: 'displaySettings',
          value: settings.display,
        });
      }

      // Return updated settings
      return this.getSettings();
    } catch (e) {
      logger.error('Failed to update settings', e);
      throw e;
    }
  }
}

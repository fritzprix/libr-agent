import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import {
  DEFAULT_SETTING,
  type ISettingsService,
  type Settings,
  type ServiceConfig,
  type ModelChoice,
  type AdvancedSettings,
  type DisplaySettings,
  type SystemSettings,
} from './settings-service';
import type { AIServiceProvider } from '@/lib/ai-service';

const logger = getLogger('RustSettingsService');

// Define specific types for each setting value
type SettingValue =
  | Record<AIServiceProvider, ServiceConfig> // serviceConfigs
  | ModelChoice // preferredModel / fallbackModel
  | null // fallbackModel can be null (cleared)
  | number // windowSize, toolCallGroupVisibleCount
  | string // uiLanguage, agentHubUrl
  | AdvancedSettings // advancedSettings
  | DisplaySettings // displaySettings
  | SystemSettings // systemSettings
  | undefined; // agentHubUrl can be undefined

interface SettingDto {
  key: string;
  value: SettingValue;
  createdAt: number;
  updatedAt: number;
}

export class RustSettingsService implements ISettingsService {
  async getSettings(): Promise<Settings> {
    try {
      const dtos = await safeInvoke<SettingDto[]>('list_settings');

      // Convert list of settings to Settings object
      const settingsMap = new Map<string, SettingValue>();
      dtos.forEach((dto) => {
        settingsMap.set(dto.key, dto.value);
      });

      // Helper function to safely get typed value with fallback
      const getTypedValue = <T extends SettingValue>(
        key: string,
        defaultValue: T,
        validator?: (val: unknown) => val is T,
      ): T => {
        const value = settingsMap.get(key);
        if (value !== undefined) {
          if (validator && !validator(value)) {
            logger.warn(
              `Invalid value for setting key: ${key}, using default`,
              { value, defaultValue },
            );
            return defaultValue;
          }
          return value as T;
        }
        return defaultValue;
      };

      // Construct settings object with defaults
      const storedSystem = getTypedValue(
        'systemSettings',
        DEFAULT_SETTING.system,
      );

      const settings: Settings = {
        ...DEFAULT_SETTING,
        serviceConfigs: {
          ...DEFAULT_SETTING.serviceConfigs,
          ...getTypedValue('serviceConfigs', DEFAULT_SETTING.serviceConfigs),
        },
        preferredModel: getTypedValue(
          'preferredModel',
          DEFAULT_SETTING.preferredModel,
        ),
        fallbackModel:
          (settingsMap.get('fallbackModel') as
            | ModelChoice
            | null
            | undefined) ?? DEFAULT_SETTING.fallbackModel,
        contextStrategy: getTypedValue(
          'contextStrategy',
          DEFAULT_SETTING.contextStrategy,
        ),
        windowSize: getTypedValue('windowSize', DEFAULT_SETTING.windowSize),
        maxInputContext: getTypedValue(
          'maxInputContext',
          DEFAULT_SETTING.maxInputContext,
        ),
        uiLanguage: getTypedValue('uiLanguage', DEFAULT_SETTING.uiLanguage),
        toolCallGroupVisibleCount: getTypedValue(
          'toolCallGroupVisibleCount',
          DEFAULT_SETTING.toolCallGroupVisibleCount,
        ),
        agentHubUrl: getTypedValue('agentHubUrl', DEFAULT_SETTING.agentHubUrl),
        advanced: getTypedValue('advancedSettings', DEFAULT_SETTING.advanced),
        display: getTypedValue('displaySettings', DEFAULT_SETTING.display),
        system: {
          ...DEFAULT_SETTING.system,
          ...storedSystem,
        },
      };

      return settings;
    } catch (error) {
      logger.error('Failed to get settings', error);
      throw error;
    }
  }

  async updateSettings(settings: Partial<Settings>): Promise<Settings> {
    try {
      // Collect all changes into a single object
      const changes: Record<string, SettingValue> = {};

      if (settings.serviceConfigs) {
        // For serviceConfigs, we need to merge with existing
        // But since we are replacing the whole object in DB, we should probably
        // fetch current first if we want to be safe, OR assume the caller passed the full object.
        // The original implementation merged.
        // Let's fetch current first to be safe.
        const currentSettings = await this.getSettings();
        const newServiceConfigs = {
          ...currentSettings.serviceConfigs,
          ...settings.serviceConfigs,
        };
        changes['serviceConfigs'] = newServiceConfigs;
      }

      if (settings.preferredModel) {
        changes['preferredModel'] = settings.preferredModel;
      }

      // fallbackModel: allow null to explicitly clear it
      if (settings.fallbackModel !== undefined) {
        changes['fallbackModel'] = settings.fallbackModel ?? null;
      }

      if (settings.contextStrategy) {
        changes['contextStrategy'] = settings.contextStrategy;
      }

      if (settings.windowSize != null) {
        changes['windowSize'] = settings.windowSize;
      }

      if (settings.maxInputContext != null) {
        changes['maxInputContext'] = settings.maxInputContext;
      }

      if (settings.uiLanguage != null) {
        changes['uiLanguage'] = settings.uiLanguage;
      }

      if (settings.toolCallGroupVisibleCount != null) {
        changes['toolCallGroupVisibleCount'] =
          settings.toolCallGroupVisibleCount;
      }

      if (settings.agentHubUrl != null) {
        changes['agentHubUrl'] = settings.agentHubUrl;
      }

      if (settings.advanced) {
        changes['advancedSettings'] = settings.advanced;
      }

      if (settings.display) {
        changes['displaySettings'] = settings.display;
      }

      if (settings.system) {
        changes['systemSettings'] = settings.system;
      }

      // Perform a single batch update
      if (Object.keys(changes).length > 0) {
        await safeInvoke<SettingDto[]>('update_settings', {
          settings: changes,
        });
      }

      // Return updated settings
      return this.getSettings();
    } catch (error) {
      logger.error('Failed to update settings', error);
      throw error;
    }
  }
}

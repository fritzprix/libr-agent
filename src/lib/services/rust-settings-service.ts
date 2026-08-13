import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import {
  DEFAULT_SETTING,
  type ISettingsService,
  type Settings,
  type ServiceConfig,
  type CustomOpenAIProvider,
  type ModelChoice,
  type AdvancedSettings,
  type DisplaySettings,
  type SystemSettings,
  type ExperimentalSettings,
} from './settings-service';
import type { AIServiceProvider } from '@/lib/ai-service';
import {
  migrateLegacyOpenAICompatibleSettings,
  normalizeCustomOpenAIProviders,
} from '@/lib/ai-service/custom-providers';

const logger = getLogger('RustSettingsService');

// Define specific types for each setting value
type SettingValue =
  | Record<AIServiceProvider, ServiceConfig> // serviceConfigs
  | CustomOpenAIProvider[] // customProviders
  | ModelChoice // preferredModel / fallbackModel
  | null // fallbackModel can be null (cleared)
  | boolean // temperatureOverrideEnabled
  | number // windowSize, toolCallGroupVisibleCount, temperature
  | string // uiLanguage, agentHubUrl
  | AdvancedSettings
  | Partial<AdvancedSettings> // advancedSettings
  | DisplaySettings // displaySettings
  | SystemSettings // systemSettings
  | ExperimentalSettings // experimentalSettings
  | undefined; // agentHubUrl can be undefined

interface SettingDto {
  key: string;
  value: SettingValue;
  createdAt: number;
  updatedAt: number;
}

let cachedSettingsValue: Settings | null = null;
let cachedSettingsPromise: Promise<Settings> | null = null;
let settingsCacheGeneration = 0;

function invalidateCachedSettingsState() {
  settingsCacheGeneration += 1;
  cachedSettingsValue = null;
  cachedSettingsPromise = null;
}

function isPartialAdvancedSettings(
  val: unknown,
): val is Partial<AdvancedSettings> {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const keys: (keyof AdvancedSettings)[] = [
    'maxRetries',
    'retryDelay',
    'diffContextLines',
    'defaultMaxOutputTokens',
    'toolResultInlineLimitBytes',
    'defaultSessionMaxDepth',
    'defaultSessionMaxFanout',
    'maxConcurrentActiveSessions',
    'maxSuspendedSessions',
    'maxConcurrentActiveProcesses',
    'maxSuspendedProcesses',
    'loopPreventionThreshold',
    'loopPreventionHardBreakOffset',
    'thinkingLoopMinPatternLength',
    'thinkingLoopMinRepetitions',
  ];
  return keys.every((key) => {
    const value = obj[key];
    return value === undefined || typeof value === 'number';
  });
}

function isCustomOpenAIProviderArray(
  val: unknown,
): val is CustomOpenAIProvider[] {
  if (!Array.isArray(val)) {
    return false;
  }
  return val.every((entry) => {
    if (typeof entry !== 'object' || entry === null) {
      return false;
    }
    const obj = entry as Record<string, unknown>;
    const apiKey = obj.apiKey;
    const models = obj.models;
    return (
      typeof obj.id === 'string' &&
      typeof obj.name === 'string' &&
      typeof obj.baseUrl === 'string' &&
      (apiKey === undefined || apiKey === null || typeof apiKey === 'string') &&
      (models === undefined ||
        models === null ||
        (Array.isArray(models) && models.every((m) => typeof m === 'string')))
    );
  });
}

function mapDtosToSettings(dtos: SettingDto[]): {
  settings: Settings;
  didMigrate: boolean;
} {
  const settingsMap = new Map<string, SettingValue>();
  dtos.forEach((dto) => {
    settingsMap.set(dto.key, dto.value);
  });

  const getTypedValue = <T extends SettingValue>(
    key: string,
    defaultValue: T,
    validator?: (val: unknown) => val is T,
  ): T => {
    const value = settingsMap.get(key);
    if (value !== undefined) {
      if (validator && !validator(value)) {
        logger.warn(`Invalid value for setting key: ${key}, using default`, {
          value,
          defaultValue,
        });
        return defaultValue;
      }
      return value as T;
    }
    return defaultValue;
  };

  const storedSystem = getTypedValue('systemSettings', DEFAULT_SETTING.system);

  const mapped: Settings = {
    ...DEFAULT_SETTING,
    serviceConfigs: {
      ...DEFAULT_SETTING.serviceConfigs,
      ...getTypedValue('serviceConfigs', DEFAULT_SETTING.serviceConfigs),
    },
    customProviders: normalizeCustomOpenAIProviders(
      getTypedValue(
        'customProviders',
        DEFAULT_SETTING.customProviders,
        isCustomOpenAIProviderArray,
      ),
    ),
    preferredModel: getTypedValue(
      'preferredModel',
      DEFAULT_SETTING.preferredModel,
    ),
    fallbackModel:
      (settingsMap.get('fallbackModel') as ModelChoice | null | undefined) ??
      DEFAULT_SETTING.fallbackModel,
    temperatureOverrideEnabled: getTypedValue(
      'temperatureOverrideEnabled',
      DEFAULT_SETTING.temperatureOverrideEnabled,
      (val): val is boolean => typeof val === 'boolean',
    ),
    temperature: (() => {
      const value = getTypedValue(
        'temperature',
        DEFAULT_SETTING.temperature,
        (val): val is number => typeof val === 'number' && Number.isFinite(val),
      );
      return Math.min(2, Math.max(0, value));
    })(),
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
    advanced: {
      ...DEFAULT_SETTING.advanced,
      ...getTypedValue<Partial<AdvancedSettings>>(
        'advancedSettings',
        {},
        isPartialAdvancedSettings,
      ),
    },
    display: getTypedValue('displaySettings', DEFAULT_SETTING.display),
    system: {
      ...DEFAULT_SETTING.system,
      ...storedSystem,
    },
    experimental: getTypedValue(
      'experimentalSettings',
      DEFAULT_SETTING.experimental,
    ),
  };

  return migrateLegacyOpenAICompatibleSettings(mapped);
}

function invalidateSettingsCache() {
  invalidateCachedSettingsState();
}

async function loadSettings(forceRefresh = false): Promise<Settings> {
  if (forceRefresh) {
    invalidateCachedSettingsState();
  }

  if (!forceRefresh && cachedSettingsValue) {
    return cachedSettingsValue;
  }

  if (!forceRefresh && cachedSettingsPromise) {
    return cachedSettingsPromise;
  }

  const requestGeneration = settingsCacheGeneration;
  const request = safeInvoke<SettingDto[]>('list_settings')
    .then(async (dtos) => {
      const { settings, didMigrate } = mapDtosToSettings(dtos);

      if (didMigrate) {
        try {
          await safeInvoke<SettingDto[]>('update_settings', {
            settings: {
              serviceConfigs: settings.serviceConfigs,
              customProviders: settings.customProviders,
              preferredModel: settings.preferredModel,
              fallbackModel: settings.fallbackModel ?? null,
            },
          });
        } catch (error) {
          logger.warn(
            'Failed to persist legacy OpenAI-compatible settings migration',
            error,
          );
        }
      }

      if (
        requestGeneration === settingsCacheGeneration &&
        cachedSettingsPromise === request
      ) {
        cachedSettingsValue = settings;
      }
      return settings;
    })
    .finally(() => {
      if (cachedSettingsPromise === request) {
        cachedSettingsPromise = null;
      }
    });

  cachedSettingsPromise = request;
  return request;
}

export function __resetRustSettingsServiceCacheForTests() {
  invalidateSettingsCache();
}

export class RustSettingsService implements ISettingsService {
  async getSettings(): Promise<Settings> {
    try {
      return await loadSettings();
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

      if (settings.customProviders !== undefined) {
        changes['customProviders'] = normalizeCustomOpenAIProviders(
          settings.customProviders,
        );
      }

      if (settings.preferredModel) {
        changes['preferredModel'] = settings.preferredModel;
      }

      // fallbackModel: allow null to explicitly clear it
      if (settings.fallbackModel !== undefined) {
        changes['fallbackModel'] = settings.fallbackModel ?? null;
      }

      if (settings.temperatureOverrideEnabled !== undefined) {
        changes['temperatureOverrideEnabled'] =
          settings.temperatureOverrideEnabled;
      }

      if (settings.temperature !== undefined) {
        changes['temperature'] = settings.temperature;
      }

      if (settings.contextStrategy != null) {
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

      if (settings.experimental) {
        changes['experimentalSettings'] = settings.experimental;
      }

      // Perform a single batch update
      if (Object.keys(changes).length > 0) {
        invalidateCachedSettingsState();
        await safeInvoke<SettingDto[]>('update_settings', {
          settings: changes,
        });
        return await loadSettings(true);
      }

      return await this.getSettings();
    } catch (error) {
      logger.error('Failed to update settings', error);
      throw error;
    }
  }
}

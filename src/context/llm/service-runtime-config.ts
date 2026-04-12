import type { AIServiceConfig } from '@/lib/ai-service/types';
import type { Settings } from '@/lib/services/settings-service';

type ConfigurableAIService = {
  setDefaultConfig?: (config: AIServiceConfig) => void;
};

export function buildServiceRuntimeConfig(
  settings: Settings,
  baseConfig: AIServiceConfig = {},
  overrides: AIServiceConfig = {},
): AIServiceConfig {
  return {
    ...baseConfig,
    maxRetries: settings.advanced.maxRetries,
    retryDelay: settings.advanced.retryDelay,
    ...overrides,
  };
}

export function applyServiceRuntimeConfig(
  service: unknown,
  config: AIServiceConfig,
): void {
  if (
    typeof service === 'object' &&
    service !== null &&
    'setDefaultConfig' in service &&
    typeof (service as ConfigurableAIService).setDefaultConfig === 'function'
  ) {
    (service as ConfigurableAIService).setDefaultConfig?.(config);
  }
}

import type { AIServiceConfig } from '@/lib/ai-service/types';
import type { Settings } from '@/lib/services/settings-service';

export function buildServiceRuntimeConfig(
  settings: Settings,
  baseConfig: AIServiceConfig = {},
  overrides: AIServiceConfig = {},
): AIServiceConfig {
  return {
    ...baseConfig,
    maxRetries: settings.advanced.maxRetries,
    retryDelay: settings.advanced.retryDelay,
    ...(settings.temperatureOverrideEnabled
      ? { temperature: settings.temperature }
      : {}),
    ...(settings.advanced.thinkingBudget !== undefined && {
      thinkingBudget: settings.advanced.thinkingBudget,
    }),
    ...overrides,
  };
}

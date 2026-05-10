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
    ...overrides,
  };
}

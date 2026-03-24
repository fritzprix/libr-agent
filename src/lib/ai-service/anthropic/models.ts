import type { AIServiceConfig } from '../types';
import type { ModelInfo } from '../../llm-config-manager';
import { llmConfigManager } from '../../llm-config-manager';

export const ANTHROPIC_MODEL_CACHE_TTL = 3600000;
export const ANTHROPIC_FALLBACK_MODEL = 'claude-3-5-sonnet-20241022';

export function isAnthropicModelCacheValid(
  cacheTimestamp?: number,
  ttl: number = ANTHROPIC_MODEL_CACHE_TTL,
): boolean {
  if (!cacheTimestamp) {
    return false;
  }

  return Date.now() - cacheTimestamp < ttl;
}

export function getDefaultAnthropicModel(config?: AIServiceConfig): string {
  if (config?.defaultModel) {
    return config.defaultModel;
  }

  const configModels = llmConfigManager.getModelsForProvider('anthropic');
  if (configModels && Object.keys(configModels).length > 0) {
    return Object.keys(configModels)[0];
  }

  return ANTHROPIC_FALLBACK_MODEL;
}

export function validateAnthropicFallbackModel(
  logger: {
    error: (message: string, ...args: unknown[]) => void;
    debug: (message: string, ...args: unknown[]) => void;
  },
  fallbackModel: string = ANTHROPIC_FALLBACK_MODEL,
): void {
  const model = llmConfigManager.getModel('anthropic', fallbackModel);
  if (!model) {
    logger.error(
      `Fallback model ${fallbackModel} not found in config. Update getDefaultModel() to use a valid fallback.`,
    );
    return;
  }

  logger.debug(`Fallback model ${fallbackModel} validated successfully`);
}

export function cacheAnthropicModels(models: ModelInfo[]): {
  modelCache: ModelInfo[];
  cacheTimestamp: number;
} {
  return {
    modelCache: models,
    cacheTimestamp: Date.now(),
  };
}

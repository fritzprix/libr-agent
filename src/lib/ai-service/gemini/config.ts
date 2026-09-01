import { HarmCategory, HarmBlockThreshold } from '@google/genai';
import { ModelInfo, llmConfigManager } from '../../llm-config-manager';
import { AIServiceConfig } from '../types';

/**
 * Check if a model supports thinking mode
 * @param modelId The ID of the model.
 * @param modelCache The cache object containing model properties.
 */
export async function checkThinkingSupport(
  modelId: string,
  modelCache?: ModelInfo[],
): Promise<boolean> {
  // Check cache first
  if (modelCache) {
    const cachedModel = modelCache.find((m) => m.id === modelId);
    if (cachedModel) {
      return cachedModel.supportReasoning;
    }
  }

  // Check static config
  const staticModel = llmConfigManager.getModel('gemini', modelId);
  if (staticModel?.supportReasoning !== undefined) {
    return staticModel.supportReasoning;
  }

  // Fallback to pattern matching
  return /gemini-2\.[5-9]|gemini-[3-9]/.test(modelId);
}

/**
 * Returns the default safety settings for Gemini.
 * Disables blocking for main harm categories to reduce false positives for agent workflows.
 */
export function getDefaultSafetySettings(): Array<{
  category: HarmCategory;
  threshold: HarmBlockThreshold;
}> {
  return [
    {
      category: HarmCategory.HARM_CATEGORY_HARASSMENT,
      threshold: HarmBlockThreshold.BLOCK_NONE,
    },
    {
      category: HarmCategory.HARM_CATEGORY_HATE_SPEECH,
      threshold: HarmBlockThreshold.BLOCK_NONE,
    },
    {
      category: HarmCategory.HARM_CATEGORY_SEXUALLY_EXPLICIT,
      threshold: HarmBlockThreshold.BLOCK_NONE,
    },
    {
      category: HarmCategory.HARM_CATEGORY_DANGEROUS_CONTENT,
      threshold: HarmBlockThreshold.BLOCK_NONE,
    },
  ];
}

/**
 * Prepares the safety settings based on the provided configuration or defaults.
 * @param config Optional AI service configuration.
 */
export function prepareSafetySettings(
  config: AIServiceConfig,
): Array<{ category: HarmCategory; threshold: HarmBlockThreshold }> {
  if (config.safetySettings) {
    return config.safetySettings as Array<{
      category: HarmCategory;
      threshold: HarmBlockThreshold;
    }>;
  }
  return getDefaultSafetySettings();
}

import { HarmCategory, HarmBlockThreshold } from '@google/genai';
import { AIServiceConfig } from '../types';

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

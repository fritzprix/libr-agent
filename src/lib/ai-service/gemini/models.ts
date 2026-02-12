import { GoogleGenAI } from '@google/genai';
import { ModelInfo, llmConfigManager } from '../../llm-config-manager';
import { getLogger } from '../../logger';

const logger = getLogger('GeminiModelManager');

/**
 * Fetches the list of available models from the Gemini API.
 * Uses pagination to retrieve all models.
 */
export async function fetchGeminiModels(
  genAI: GoogleGenAI,
): Promise<ModelInfo[]> {
  logger.info('Fetching models from Gemini API...');

  // Call Gemini API models.list() with pagination
  // Note: We need to handle retry logic in the caller or pass a retry function
  // For simplicity, we assume the caller handles retries or we just call the API directly here.
  // If we want to use the service's withRetry, we might need to pass it.
  // But usually, network calls should be retried.
  // Let's assume the caller wraps this in a retry block if needed,
  // or we can implement a simple retry here.

  const pager = await genAI.models.list({
    config: {
      pageSize: 100, // Fetch up to 100 models per page
    },
  });

  const models: ModelInfo[] = [];
  let totalFetched = 0;
  let filteredOut = 0;

  // Iterate through all pages using AsyncIterable
  for await (const geminiModel of pager) {
    totalFetched++;

    // Extract model name (remove 'models/' prefix if present)
    const modelId = geminiModel.name?.replace(/^models\//, '') || '';

    if (!modelId) {
      logger.warn('Skipping model with empty ID', { geminiModel });
      filteredOut++;
      continue;
    }

    // Filter: Only include models that support generateContent
    const supportsGeneration =
      geminiModel.supportedActions?.includes('generateContent') ?? true;

    if (!supportsGeneration) {
      logger.debug('Skipping non-generation model', {
        modelId,
        supportedActions: geminiModel.supportedActions,
      });
      filteredOut++;
      continue;
    }

    logger.debug('Processing model from API', {
      modelId,
      displayName: geminiModel.displayName,
      inputTokenLimit: geminiModel.inputTokenLimit,
      supportedActions: geminiModel.supportedActions,
    });

    // Merge with static config metadata
    const staticModel = llmConfigManager.getModel('gemini', modelId);

    // Use API-provided context window with fallback
    const contextWindow =
      geminiModel.inputTokenLimit || staticModel?.contextWindow || 1048576; // Default to 1M tokens

    const modelInfo: ModelInfo = {
      id: modelId,
      name: geminiModel.displayName || staticModel?.name || modelId,
      contextWindow,
      // Detect thinking mode support from API response or model name
      supportReasoning:
        staticModel?.supportReasoning ??
        /gemini-2\.[5-9]|gemini-[3-9]/.test(modelId),
      supportTools: staticModel?.supportTools ?? true,
      supportStreaming: staticModel?.supportStreaming ?? true,
      cost: staticModel?.cost || { input: 0, output: 0 },
      description:
        geminiModel.description ||
        staticModel?.description ||
        `Gemini model: ${modelId}`,
    };

    models.push(modelInfo);
  }

  // Add static config models that aren't in the API response
  const staticModels = llmConfigManager.getModelsForProvider('gemini');
  if (staticModels) {
    const apiModelIds = new Set(models.map((m) => m.id));
    const staticModelIds = Object.keys(staticModels);

    for (const staticId of staticModelIds) {
      if (!apiModelIds.has(staticId)) {
        const staticModel = staticModels[staticId];
        logger.debug('Adding static-only model', {
          modelId: staticId,
          name: staticModel.name,
        });

        models.push({
          id: staticId,
          name: staticModel.name,
          contextWindow: staticModel.contextWindow,
          supportReasoning: staticModel.supportReasoning,
          supportTools: staticModel.supportTools,
          supportStreaming: staticModel.supportStreaming,
          cost: staticModel.cost,
          description: staticModel.description,
        });
      }
    }
  }

  logger.info(
    `Loaded ${models.length} total models (API: ${models.length - (staticModels ? Object.keys(staticModels).length - totalFetched + filteredOut : 0)}, static-only: ${staticModels ? Object.keys(staticModels).length - (models.length - filteredOut) : 0})`,
  );
  return models;
}

/**
 * Get the default model from static config
 */
export function getDefaultModel(): string {
  // Try to get from static config first
  const staticModels = llmConfigManager.getModelsForProvider('gemini');
  if (staticModels) {
    const modelIds = Object.keys(staticModels);
    // Prefer Gemini 2.5 Flash as default (fast & capable)
    const preferred = modelIds.find((id) => id.includes('gemini-2.5-flash'));
    if (preferred) return preferred;

    // Fallback to first available model
    if (modelIds.length > 0) return modelIds[0];
  }

  // Ultimate fallback
  return 'gemini-1.5-pro';
}

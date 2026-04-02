import OpenAI from 'openai';

import { llmConfigManager, type ModelInfo } from '@/lib/llm-config-manager';

import { getContextWindow } from '../model-capabilities';
import { AIServiceProvider } from '../types';
import type { OpenAILoggerLike } from './types';

interface OpenAIModelsAPI {
  models: { list: () => Promise<{ data: unknown[] }> };
}

export async function fetchOpenAIModels(args: {
  openai: OpenAI;
  provider: AIServiceProvider;
  withRetry: <T>(fn: () => Promise<T>) => Promise<T>;
  logger: OpenAILoggerLike;
}): Promise<ModelInfo[]> {
  args.logger.info('Fetching models from OpenAI...');

  const response = await args.withRetry(async () => {
    const openaiClient = args.openai as unknown as OpenAIModelsAPI;
    return openaiClient.models.list();
  });

  const modelsRaw: Array<unknown> = Array.isArray(response?.data)
    ? (response.data as Array<unknown>)
    : [];

  const modelPromises = modelsRaw.map(async (entry) => {
    if (entry == null || typeof entry !== 'object') return null;
    const modelEntry = entry as Record<string, unknown>;

    const id =
      (typeof modelEntry.id === 'string' && modelEntry.id) ||
      (typeof modelEntry.model === 'string' && modelEntry.model) ||
      (typeof modelEntry.name === 'string' && modelEntry.name) ||
      String(modelEntry);

    const staticModel = llmConfigManager.getModel('openai', id);
    const contextWindow = await getContextWindow(id, args.provider);

    const name = staticModel?.name || id;
    const supportStreaming = staticModel?.supportStreaming ?? true;
    const supportReasoning =
      staticModel?.supportReasoning ??
      (id.toLowerCase().includes('gpt-4') ||
        id.toLowerCase().includes('gpt-3.5'));
    const supportTools = staticModel?.supportTools ?? false;

    const description =
      staticModel?.description ||
      (typeof modelEntry.description === 'string' && modelEntry.description) ||
      (Array.isArray(modelEntry.permission)
        ? modelEntry.permission.join(',')
        : undefined) ||
      id;

    const modelInfo: ModelInfo = {
      id,
      name,
      contextWindow,
      supportReasoning,
      supportTools,
      supportStreaming,
      cost: staticModel?.cost || { input: 0, output: 0 },
      description,
    };

    return modelInfo;
  });

  const models = (await Promise.all(modelPromises)).filter(
    (value): value is ModelInfo => value !== null,
  );

  args.logger.info(`Loaded ${models.length} models from OpenAI API`);
  return models;
}

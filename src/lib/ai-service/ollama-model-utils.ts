import { noopLogger, type Logger } from './ollama-core-types';

export function getModelToolSupport(modelName: string): boolean {
  const toolSupportModels = [
    'llama3.1',
    'llama3.2',
    'qwen',
    'mistral',
    'dolphin',
    'deepseek',
  ];

  const baseName = modelName.split(':')[0].toLowerCase();
  return toolSupportModels.some((model) => baseName.includes(model));
}

export function determineThinkParam(
  enableReasoning: boolean,
  reasoningEffort?: 'low' | 'medium' | 'high',
  modelSupportsThinking: boolean = true,
  logger: Logger = noopLogger,
): boolean | 'low' | 'medium' | 'high' | undefined {
  if (!enableReasoning) {
    return undefined;
  }

  if (!modelSupportsThinking) {
    logger.debug('Model may not support thinking, but will try anyway');
  }

  const thinkParam = reasoningEffort || true;
  logger.info('Reasoning mode enabled', {
    thinkParam,
    modelSupportsThinking,
  });

  return thinkParam;
}

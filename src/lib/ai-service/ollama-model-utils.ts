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

import { AIServiceProvider } from './types';

interface DynamicModelFetchPolicyArgs {
  provider?: string;
  apiKey?: string;
  use3rdParty?: boolean;
  customModelId?: string;
}

export function shouldFetchDynamicModels({
  provider,
  apiKey,
  use3rdParty,
  customModelId,
}: DynamicModelFetchPolicyArgs): boolean {
  if (!provider) {
    return false;
  }

  if (
    provider === AIServiceProvider.OpenAI &&
    use3rdParty &&
    customModelId?.trim()
  ) {
    return false;
  }

  if (
    provider === AIServiceProvider.Ollama ||
    provider === AIServiceProvider.OpenRouter
  ) {
    return true;
  }

  return Boolean(apiKey?.trim());
}

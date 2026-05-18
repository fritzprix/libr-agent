import { AIServiceProvider } from './types';

interface DynamicModelFetchPolicyArgs {
  provider?: string;
  apiKey?: string;
  use3rdParty?: boolean;
  customModelId?: string;
}

export type DynamicModelFetchPolicyReason =
  | 'missing-provider'
  | 'missing-api-key'
  | 'custom-openai-model'
  | 'allowed';

export interface DynamicModelFetchPolicyDecision {
  canFetch: boolean;
  reason: DynamicModelFetchPolicyReason;
}

export function getDynamicModelFetchPolicy({
  provider,
  apiKey,
  use3rdParty,
  customModelId,
}: DynamicModelFetchPolicyArgs): DynamicModelFetchPolicyDecision {
  if (!provider) {
    return {
      canFetch: false,
      reason: 'missing-provider',
    };
  }

  if (
    provider === AIServiceProvider.OpenAI &&
    use3rdParty &&
    customModelId?.trim()
  ) {
    return {
      canFetch: false,
      reason: 'custom-openai-model',
    };
  }

  if (
    provider === AIServiceProvider.Ollama ||
    provider === AIServiceProvider.OpenRouter
  ) {
    return {
      canFetch: true,
      reason: 'allowed',
    };
  }

  if (apiKey?.trim()) {
    return {
      canFetch: true,
      reason: 'allowed',
    };
  }

  return {
    canFetch: false,
    reason: 'missing-api-key',
  };
}

export function shouldFetchDynamicModels({
  provider,
  apiKey,
  use3rdParty,
  customModelId,
}: DynamicModelFetchPolicyArgs): boolean {
  return getDynamicModelFetchPolicy({
    provider,
    apiKey,
    use3rdParty,
    customModelId,
  }).canFetch;
}

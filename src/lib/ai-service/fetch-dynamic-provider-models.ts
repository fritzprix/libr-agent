/**
 * Shared dynamic model listing for pickers / ModelProvider.
 * Keeps timeout, toast policy, and SWR credential fingerprinting in one place.
 */

import { resolveProviderRuntimeConfig } from '@/lib/ai-service/custom-providers';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import { stableHashKeyPart } from '@/lib/ai-service/base-service-utils';
import { reportListModelsFallback } from '@/lib/ai-service/list-models-errors';
import { getDynamicModelFetchPolicy } from '@/lib/ai-service/model-fetch-policy';
import { setStoredModelCache } from '@/lib/ai-service/model-cache-storage';
import {
  resolveProviderModels,
  type ProviderModelMap,
} from '@/lib/ai-service/resolve-provider-models';
import type { AIModelLookupService } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import { withTimeout } from '@/lib/retry-utils';
import type { Settings } from '@/lib/services/settings-service';

const logger = getLogger('fetchDynamicProviderModels');

/** Background discovery: fail fast so one offline provider cannot stall the UI. */
export const BACKGROUND_LIST_MODELS_TIMEOUT_MS = 8_000;
/** Explicit Refresh: allow slower endpoints. */
export const REFRESH_LIST_MODELS_TIMEOUT_MS = 20_000;

export type ProviderModelsSettingsSlice = Pick<
  Settings,
  'serviceConfigs' | 'customProviders'
>;

export interface FetchDynamicProviderModelsOptions {
  timeoutMs: number;
  notifyUser: boolean;
}

/**
 * Fingerprint an API key for SWR cache keys without storing the raw secret.
 */
export function fingerprintCredentialForSwrKey(apiKey: string): string {
  if (!apiKey) {
    return '';
  }
  return stableHashKeyPart(apiKey);
}

/**
 * Stable per-provider segment for model-list SWR keys.
 * Format: `providerId|apiKeyFingerprint|baseUrl|party|customModelId`
 */
export function buildProviderModelsSwrSegment(args: {
  providerId: string;
  apiKey: string;
  baseUrl?: string | null;
  use3rdParty?: boolean;
  customModelId?: string | null;
}): string {
  return [
    args.providerId,
    fingerprintCredentialForSwrKey(args.apiKey),
    args.baseUrl ?? '',
    args.use3rdParty ? 'use-3rd-party' : 'first-party',
    args.customModelId ?? '',
  ].join('|');
}

/**
 * Fetch `/models` (or provider equivalent) for one provider.
 * On failure: silent or toast via `notifyUser`, then return `{}` so callers
 * can fall back through resolveProviderModels / local cache.
 */
export async function fetchDynamicProviderModels(
  providerId: string,
  settings: ProviderModelsSettingsSlice,
  options: FetchDynamicProviderModelsOptions,
): Promise<ProviderModelMap> {
  const resolved = resolveProviderRuntimeConfig(providerId, settings);
  const policy = getDynamicModelFetchPolicy({
    provider: providerId,
    apiKey: resolved.apiKey,
    baseUrl: resolved.baseUrl,
    use3rdParty: resolved.use3rdParty,
    customModelId: resolved.customModelId,
  });

  if (!policy.canFetch) {
    return {};
  }

  const effectiveApiKey = resolved.apiKey || 'no-api-key';

  try {
    const service: AIModelLookupService = AIServiceFactory.getService(
      providerId,
      effectiveApiKey,
      resolved.serviceConfig,
    );
    const modelList = await withTimeout(
      service.listModels(),
      options.timeoutMs,
    );

    const modelsRecord = modelList.reduce<ProviderModelMap>(
      (acc, modelInfo) => {
        const key = modelInfo.id || modelInfo.name;
        acc[key] = modelInfo;
        return acc;
      },
      {},
    );

    if (Object.keys(modelsRecord).length > 0) {
      setStoredModelCache(providerId, modelsRecord);
    }

    logger.debug(
      `Fetched ${Object.keys(modelsRecord).length} models from ${providerId}`,
    );
    return modelsRecord;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.warn(`Failed to fetch models for ${providerId}:`, errorMessage);
    const fallbackModels = resolveProviderModels(providerId, settings);
    reportListModelsFallback({
      provider: providerId,
      baseUrl: resolved.baseUrl,
      reason: 'api_error',
      error,
      hasCachedModels: Object.keys(fallbackModels).length > 0,
      notifyUser: options.notifyUser,
    });
    return {};
  }
}

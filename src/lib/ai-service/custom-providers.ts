import { createId } from '@paralleldrive/cuid2';
import type {
  CustomOpenAIProvider,
  ServiceConfig,
  Settings,
} from '@/lib/services/settings-service';
import { AIServiceProvider, type AIServiceConfig } from './types';
import { llmConfigManager } from '@/lib/llm-config-manager';

export const CUSTOM_PROVIDER_PREFIX = 'custom:';

export interface ResolvedProviderRuntimeConfig {
  /** Value passed to AIServiceFactory (builtin enum or OpenAI for custom). */
  factoryProvider: AIServiceProvider;
  /** Session / settings provider id (builtin or `custom:<id>`). */
  providerId: string;
  displayName: string;
  apiKey: string;
  baseUrl?: string;
  use3rdParty?: boolean;
  customModelId?: string;
  /** Manual model IDs for custom providers when /v1/models is unavailable. */
  manualModels?: string[];
  /** Config object suitable for AIServiceFactory.getService. */
  serviceConfig: AIServiceConfig;
}

/**
 * Returns true when `providerId` is a custom OpenAI-compatible provider
 * (`custom:<non-empty-id>`).
 *
 * Nullish values are treated as non-custom (returns false) so call sites can
 * pass optional provider strings without an extra guard.
 */
export function isCustomOpenAIProviderId(
  providerId: string | undefined | null,
): boolean {
  return (
    typeof providerId === 'string' &&
    providerId.startsWith(CUSTOM_PROVIDER_PREFIX) &&
    providerId.length > CUSTOM_PROVIDER_PREFIX.length
  );
}

export function toCustomProviderId(id: string): string {
  if (isCustomOpenAIProviderId(id)) {
    return id;
  }
  return `${CUSTOM_PROVIDER_PREFIX}${id}`;
}

export function parseCustomProviderId(providerId: string): string | undefined {
  if (!isCustomOpenAIProviderId(providerId)) {
    return undefined;
  }
  return providerId.slice(CUSTOM_PROVIDER_PREFIX.length);
}

export function normalizeManualModels(
  models: string[] | null | undefined,
): string[] | undefined {
  if (!models || models.length === 0) {
    return undefined;
  }
  const cleaned = models.map((m) => m.trim()).filter((m) => m.length > 0);
  return cleaned.length > 0 ? cleaned : undefined;
}

/**
 * Canonicalize a custom provider for persistence / equality checks.
 * Optional empty fields are omitted so JSON round-trips match in-memory form state.
 */
export function normalizeCustomOpenAIProvider(
  provider: CustomOpenAIProvider,
): CustomOpenAIProvider {
  const apiKey = provider.apiKey?.trim();
  const models = normalizeManualModels(provider.models);
  const normalized: CustomOpenAIProvider = {
    id: provider.id.trim(),
    name: provider.name.trim(),
    baseUrl: provider.baseUrl.trim(),
  };
  if (apiKey) {
    normalized.apiKey = apiKey;
  }
  if (models) {
    normalized.models = models;
  }
  return normalized;
}

export function normalizeCustomOpenAIProviders(
  providers: CustomOpenAIProvider[] | null | undefined,
): CustomOpenAIProvider[] {
  if (!providers || providers.length === 0) {
    return [];
  }
  return providers
    .filter(
      (provider) => typeof provider?.id === 'string' && provider.id.trim(),
    )
    .map(normalizeCustomOpenAIProvider);
}

export function createCustomOpenAIProvider(
  partial: Omit<CustomOpenAIProvider, 'id'> & { id?: string },
): CustomOpenAIProvider {
  return normalizeCustomOpenAIProvider({
    id: partial.id ?? createId(),
    name: partial.name,
    baseUrl: partial.baseUrl,
    apiKey: partial.apiKey,
    models: partial.models,
  });
}

export function findCustomOpenAIProvider(
  settings: Pick<Settings, 'customProviders'>,
  providerId: string,
): CustomOpenAIProvider | undefined {
  const id = parseCustomProviderId(providerId);
  if (!id) {
    return undefined;
  }
  return (settings.customProviders ?? []).find((p) => p.id === id);
}

function builtinDisplayName(provider: AIServiceProvider): string {
  return llmConfigManager.getProvider(provider)?.name ?? provider;
}

/**
 * Resolves API credentials and factory routing for a builtin or custom provider id.
 */
export function resolveProviderRuntimeConfig(
  providerId: string,
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
): ResolvedProviderRuntimeConfig {
  if (isCustomOpenAIProviderId(providerId)) {
    const entry = findCustomOpenAIProvider(settings, providerId);
    const apiKey = entry?.apiKey ?? '';
    const baseUrl = entry?.baseUrl ?? '';
    const serviceConfig: AIServiceConfig = {
      baseUrl: baseUrl || undefined,
      use3rdParty: true,
    };

    return {
      factoryProvider: AIServiceProvider.OpenAI,
      providerId,
      displayName: entry?.name ?? providerId,
      apiKey,
      baseUrl: baseUrl || undefined,
      use3rdParty: true,
      manualModels: entry?.models,
      serviceConfig,
    };
  }

  const provider = providerId as AIServiceProvider;
  const cfg: ServiceConfig = settings.serviceConfigs?.[provider] ?? {};
  const serviceConfig: AIServiceConfig = {
    baseUrl: cfg.baseUrl,
    use3rdParty: cfg.use3rdParty,
    customModelId: cfg.customModelId,
    safetySettings: cfg.safetySettings,
  };

  return {
    factoryProvider: Object.values(AIServiceProvider).includes(provider)
      ? provider
      : AIServiceProvider.Empty,
    providerId,
    displayName: builtinDisplayName(provider),
    apiKey: cfg.apiKey ?? '',
    baseUrl: cfg.baseUrl,
    use3rdParty: cfg.use3rdParty,
    customModelId: cfg.customModelId,
    serviceConfig,
  };
}

export function listCustomProviderPickerOptions(
  customProviders: CustomOpenAIProvider[] | undefined,
): Array<{ label: string; value: string }> {
  return (customProviders ?? []).map((p) => ({
    label: p.name || toCustomProviderId(p.id),
    value: toCustomProviderId(p.id),
  }));
}

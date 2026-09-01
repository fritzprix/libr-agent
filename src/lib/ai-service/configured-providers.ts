import { llmConfigManager } from '@/lib/llm-config-manager';
import type {
  CustomOpenAIProvider,
  ServiceConfig,
  Settings,
} from '@/lib/services/settings-service';
import { AIServiceProvider } from './types';
import {
  findCustomOpenAIProvider,
  isCustomOpenAIProviderId,
  toCustomProviderId,
} from './custom-providers';

export interface ProviderPickerGroup {
  providerId: string;
  label: string;
}

function isBuiltinProviderConfigured(
  provider: AIServiceProvider,
  cfg: ServiceConfig,
): boolean {
  if (
    provider === AIServiceProvider.OpenAI &&
    cfg.use3rdParty &&
    cfg.customModelId?.trim()
  ) {
    return true;
  }

  if (cfg.apiKey?.trim()) {
    return true;
  }

  if (provider === AIServiceProvider.Ollama && cfg.baseUrl?.trim()) {
    return true;
  }

  return false;
}

/**
 * Returns true when a provider has enough configuration to appear in pickers.
 * Unconfigured built-ins and custom providers are hidden.
 */
export function isProviderConfigured(
  providerId: string,
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
): boolean {
  if (isCustomOpenAIProviderId(providerId)) {
    const entry = findCustomOpenAIProvider(settings, providerId);
    return Boolean(entry?.baseUrl?.trim());
  }

  const provider = providerId as AIServiceProvider;
  if (!Object.values(AIServiceProvider).includes(provider)) {
    return false;
  }

  const cfg = settings.serviceConfigs?.[provider] ?? {};
  return isBuiltinProviderConfigured(provider, cfg);
}

/**
 * Lists configured built-in and custom providers for grouped model pickers.
 */
export function listConfiguredProviderGroups(
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
): ProviderPickerGroup[] {
  const groups: ProviderPickerGroup[] = [];

  for (const provider of Object.values(AIServiceProvider)) {
    if (provider === AIServiceProvider.Empty) {
      continue;
    }
    if (!isProviderConfigured(provider, settings)) {
      continue;
    }
    groups.push({
      providerId: provider,
      label: llmConfigManager.getProvider(provider)?.name ?? provider,
    });
  }

  for (const customProvider of settings.customProviders ?? []) {
    const providerId = toCustomProviderId(customProvider.id);
    if (!isProviderConfigured(providerId, settings)) {
      continue;
    }
    groups.push({
      providerId,
      label: customProvider.name || providerId,
    });
  }

  return groups;
}

export function buildSettingsSnapshot(
  serviceConfigs: Settings['serviceConfigs'],
  customProviders: CustomOpenAIProvider[] | undefined,
): Pick<Settings, 'serviceConfigs' | 'customProviders'> {
  return {
    serviceConfigs,
    customProviders: customProviders ?? [],
  };
}

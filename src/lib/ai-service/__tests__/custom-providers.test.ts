import { describe, expect, it } from 'vitest';
import { AIServiceProvider } from '../types';
import {
  createCustomOpenAIProvider,
  isCustomOpenAIProviderId,
  migrateLegacyOpenAICompatibleSettings,
  parseCustomProviderId,
  resolveDefaultModelForProviderChange,
  resolveProviderRuntimeConfig,
  toCustomProviderId,
} from '../custom-providers';
import {
  clearLastSelectedModel,
  setLastSelectedModel,
} from '../last-selected-model-storage';
import type { Settings } from '@/lib/services/settings-service';
import { DEFAULT_SETTING } from '@/lib/services/settings-service';

function settingsWithCustom(
  overrides: Partial<Settings> = {},
): Pick<Settings, 'serviceConfigs' | 'customProviders'> {
  return {
    serviceConfigs: {
      ...DEFAULT_SETTING.serviceConfigs,
      [AIServiceProvider.OpenAI]: {
        apiKey: 'sk-openai',
        baseUrl: 'https://api.openai.com/v1',
      },
    },
    customProviders: [
      {
        id: 'abc123',
        name: 'Local vLLM',
        baseUrl: 'http://192.168.1.10:8000/v1',
        apiKey: 'local-key',
        models: ['llama-3.1-70b'],
      },
    ],
    ...overrides,
  };
}

describe('custom provider helpers', () => {
  it('detects and parses custom provider ids', () => {
    expect(isCustomOpenAIProviderId('custom:abc123')).toBe(true);
    expect(isCustomOpenAIProviderId('openai')).toBe(false);
    expect(isCustomOpenAIProviderId('custom:')).toBe(false);
    expect(parseCustomProviderId('custom:abc123')).toBe('abc123');
    expect(toCustomProviderId('abc123')).toBe('custom:abc123');
    expect(toCustomProviderId('custom:abc123')).toBe('custom:abc123');
  });

  it('creates a custom provider with a stable id and cleaned models', () => {
    const created = createCustomOpenAIProvider({
      id: 'fixed-id',
      name: '  LM Studio  ',
      baseUrl: ' http://localhost:1234/v1 ',
      models: [' model-a ', '', 'model-b'],
    });

    expect(created).toEqual({
      id: 'fixed-id',
      name: 'LM Studio',
      baseUrl: 'http://localhost:1234/v1',
      models: ['model-a', 'model-b'],
    });
    expect(created).not.toHaveProperty('apiKey');
  });

  it('omits empty optional fields so JSON round-trips stay equal', () => {
    const created = createCustomOpenAIProvider({
      id: 'empty-optionals',
      name: 'Local',
      baseUrl: 'http://127.0.0.1:8000/v1',
      apiKey: '   ',
      models: [],
    });

    expect(created).toEqual({
      id: 'empty-optionals',
      name: 'Local',
      baseUrl: 'http://127.0.0.1:8000/v1',
    });
    expect(JSON.parse(JSON.stringify(created))).toEqual(created);
  });

  it('resolves custom providers to OpenAI factory routing', () => {
    const resolved = resolveProviderRuntimeConfig(
      'custom:abc123',
      settingsWithCustom(),
    );

    expect(resolved.factoryProvider).toBe(AIServiceProvider.OpenAI);
    expect(resolved.apiKey).toBe('local-key');
    expect(resolved.baseUrl).toBe('http://192.168.1.10:8000/v1');
    expect(resolved.use3rdParty).toBe(true);
    expect(resolved.displayName).toBe('Local vLLM');
    expect(resolved.manualModels).toEqual(['llama-3.1-70b']);
    expect(resolved.serviceConfig).toEqual({
      baseUrl: 'http://192.168.1.10:8000/v1',
      use3rdParty: true,
    });
  });

  it('resolves builtin providers from serviceConfigs', () => {
    const resolved = resolveProviderRuntimeConfig(
      AIServiceProvider.OpenAI,
      settingsWithCustom(),
    );

    expect(resolved.factoryProvider).toBe(AIServiceProvider.OpenAI);
    expect(resolved.apiKey).toBe('sk-openai');
    expect(resolved.baseUrl).toBe('https://api.openai.com/v1');
    expect(resolved.use3rdParty).toBeUndefined();
  });

  it('falls back when a custom provider id is missing from settings', () => {
    const providerId = 'custom:missing-id';
    const resolved = resolveProviderRuntimeConfig(
      providerId,
      settingsWithCustom({ customProviders: [] }),
    );

    expect(resolved.factoryProvider).toBe(AIServiceProvider.OpenAI);
    expect(resolved.providerId).toBe(providerId);
    expect(resolved.displayName).toBe(providerId);
    expect(resolved.apiKey).toBe('');
    expect(resolved.baseUrl).toBeUndefined();
    expect(resolved.use3rdParty).toBe(true);
    expect(resolved.manualModels).toBeUndefined();
  });

  it('migrates legacy openai use3rdParty config into customProviders', () => {
    const { settings, didMigrate } = migrateLegacyOpenAICompatibleSettings({
      ...DEFAULT_SETTING,
      serviceConfigs: {
        ...DEFAULT_SETTING.serviceConfigs,
        [AIServiceProvider.OpenAI]: {
          apiKey: 'sk-local',
          baseUrl: 'http://127.0.0.1:1234/v1',
          use3rdParty: true,
          customModelId: 'llama-local',
        },
      },
      preferredModel: {
        provider: AIServiceProvider.OpenAI,
        model: 'llama-local',
      },
      customProviders: [],
    });

    expect(didMigrate).toBe(true);
    expect(settings.serviceConfigs[AIServiceProvider.OpenAI]).toEqual({});
    expect(settings.customProviders).toHaveLength(1);
    expect(settings.customProviders[0]).toEqual(
      expect.objectContaining({
        name: '127.0.0.1:1234',
        baseUrl: 'http://127.0.0.1:1234/v1',
        apiKey: 'sk-local',
        models: ['llama-local'],
      }),
    );
    expect(settings.preferredModel).toEqual({
      provider: `custom:${settings.customProviders[0].id}`,
      model: 'llama-local',
    });
  });

  it('does not duplicate custom providers when migrating the same base URL twice', () => {
    const first = migrateLegacyOpenAICompatibleSettings({
      ...DEFAULT_SETTING,
      serviceConfigs: {
        ...DEFAULT_SETTING.serviceConfigs,
        [AIServiceProvider.OpenAI]: {
          baseUrl: 'http://localhost:8000/v1',
          use3rdParty: true,
          customModelId: 'model-a',
        },
      },
      customProviders: [],
    });

    const second = migrateLegacyOpenAICompatibleSettings({
      ...first.settings,
      serviceConfigs: {
        ...first.settings.serviceConfigs,
        [AIServiceProvider.OpenAI]: {
          baseUrl: 'http://localhost:8000/v1',
          use3rdParty: true,
          customModelId: 'model-b',
        },
      },
    });

    expect(second.settings.customProviders).toHaveLength(1);
    expect(second.settings.customProviders[0].models).toEqual([
      'model-a',
      'model-b',
    ]);
    expect(second.settings.serviceConfigs[AIServiceProvider.OpenAI]).toEqual(
      {},
    );
  });

  it('treats nullish provider ids as non-custom', () => {
    expect(isCustomOpenAIProviderId(null)).toBe(false);
    expect(isCustomOpenAIProviderId(undefined)).toBe(false);
    expect(isCustomOpenAIProviderId('custom:')).toBe(false);
  });

  it('resolves default model only from the target provider on switch', () => {
    clearLastSelectedModel();
    const settings = settingsWithCustom();

    expect(
      resolveDefaultModelForProviderChange(
        'custom:abc123',
        settings,
        'gpt-4o',
      ),
    ).toBe('llama-3.1-70b');

    expect(
      resolveDefaultModelForProviderChange(
        'custom:missing-id',
        settingsWithCustom({ customProviders: [] }),
        'gpt-4o',
      ),
    ).toBe('');

    expect(
      resolveDefaultModelForProviderChange(
        'custom:abc123',
        settings,
        'llama-3.1-70b',
      ),
    ).toBe('llama-3.1-70b');
  });

  it('prefers the last configured model for a provider when still valid', () => {
    clearLastSelectedModel();
    setLastSelectedModel('custom:abc123', 'llama-3.1-70b');

    expect(
      resolveDefaultModelForProviderChange(
        'custom:abc123',
        settingsWithCustom({
          customProviders: [
            {
              id: 'abc123',
              name: 'Local vLLM',
              baseUrl: 'http://192.168.1.10:8000/v1',
              models: ['other-model', 'llama-3.1-70b'],
            },
          ],
        }),
        'gpt-4o',
      ),
    ).toBe('llama-3.1-70b');
  });

  it('ignores a stale last-selected model that is no longer valid', () => {
    clearLastSelectedModel();
    setLastSelectedModel('custom:abc123', 'retired-model');

    expect(
      resolveDefaultModelForProviderChange(
        'custom:abc123',
        settingsWithCustom(),
        'gpt-4o',
      ),
    ).toBe('llama-3.1-70b');
  });
});

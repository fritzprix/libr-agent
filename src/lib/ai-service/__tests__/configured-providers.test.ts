import { describe, expect, it } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import {
  isProviderConfigured,
  listConfiguredProviderGroups,
} from '@/lib/ai-service/configured-providers';
import type { Settings } from '@/lib/services/settings-service';

const baseSettings = {
  serviceConfigs: {
    [AIServiceProvider.OpenAI]: {},
    [AIServiceProvider.Anthropic]: {},
    [AIServiceProvider.Gemini]: {},
    [AIServiceProvider.Ollama]: {},
    [AIServiceProvider.Groq]: {},
    [AIServiceProvider.Fireworks]: {},
    [AIServiceProvider.Cerebras]: {},
    [AIServiceProvider.OpenRouter]: {},
    [AIServiceProvider.Empty]: {},
  },
  customProviders: [],
} satisfies Pick<Settings, 'serviceConfigs' | 'customProviders'>;

describe('configured-providers', () => {
  it('treats providers with API keys as configured', () => {
    const settings: Pick<Settings, 'serviceConfigs' | 'customProviders'> = {
      ...baseSettings,
      serviceConfigs: {
        ...baseSettings.serviceConfigs,
        [AIServiceProvider.Anthropic]: { apiKey: 'secret' },
      },
    };

    expect(isProviderConfigured(AIServiceProvider.Anthropic, settings)).toBe(
      true,
    );
    expect(isProviderConfigured(AIServiceProvider.OpenAI, settings)).toBe(
      false,
    );
  });

  it('treats custom providers with baseUrl as configured', () => {
    const settings: Pick<Settings, 'serviceConfigs' | 'customProviders'> = {
      ...baseSettings,
      customProviders: [
        {
          id: 'local1',
          name: 'Local vLLM',
          baseUrl: 'http://127.0.0.1:8000/v1',
        },
      ],
    };

    expect(isProviderConfigured('custom:local1', settings)).toBe(true);
    expect(listConfiguredProviderGroups(settings)).toEqual([
      {
        providerId: 'custom:local1',
        label: 'Local vLLM',
      },
    ]);
  });

  it('requires explicit Ollama baseUrl before showing the provider', () => {
    const settings: Pick<Settings, 'serviceConfigs' | 'customProviders'> = {
      ...baseSettings,
      serviceConfigs: {
        ...baseSettings.serviceConfigs,
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      },
    };

    expect(isProviderConfigured(AIServiceProvider.Ollama, settings)).toBe(
      true,
    );
  });
});

import { describe, expect, it } from 'vitest';
import { AIServiceProvider } from '../types';
import {
  getDynamicModelFetchPolicy,
  shouldFetchDynamicModels,
} from '../model-fetch-policy';

describe('shouldFetchDynamicModels', () => {
  it('skips providers without enough configuration', () => {
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.OpenAI,
        apiKey: '',
      }),
    ).toBe(false);
  });

  it('allows providers with required API keys', () => {
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.OpenAI,
        apiKey: 'sk-test',
      }),
    ).toBe(true);
  });

  it('always allows ollama and openrouter discovery', () => {
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.Ollama,
        apiKey: '',
      }),
    ).toBe(true);
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.OpenRouter,
        apiKey: '',
      }),
    ).toBe(true);
  });

  it('requires an api key for first-party openai discovery', () => {
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.OpenAI,
        apiKey: '',
      }),
    ).toBe(false);
  });

  it('skips openai-compatible custom model setups that already supply a custom id', () => {
    expect(
      shouldFetchDynamicModels({
        provider: AIServiceProvider.OpenAI,
        apiKey: '',
        use3rdParty: true,
        customModelId: 'local-model',
      }),
    ).toBe(false);
  });

  it('returns a missing api key reason for providers gated by credentials', () => {
    expect(
      getDynamicModelFetchPolicy({
        provider: AIServiceProvider.Anthropic,
        apiKey: '',
      }),
    ).toEqual({
      canFetch: false,
      reason: 'missing-api-key',
    });
  });

  it('returns a custom model reason for openai-compatible custom model setups', () => {
    expect(
      getDynamicModelFetchPolicy({
        provider: AIServiceProvider.OpenAI,
        apiKey: '',
        use3rdParty: true,
        customModelId: 'local-model',
      }),
    ).toEqual({
      canFetch: false,
      reason: 'custom-openai-model',
    });
  });

  it('returns allowed for providers with dynamic discovery enabled', () => {
    expect(
      getDynamicModelFetchPolicy({
        provider: AIServiceProvider.Ollama,
        apiKey: '',
      }),
    ).toEqual({
      canFetch: true,
      reason: 'allowed',
    });
  });

  it('allows custom OpenAI-compatible providers with a base URL and no API key', () => {
    expect(
      shouldFetchDynamicModels({
        provider: 'custom:local-vllm',
        apiKey: '',
        baseUrl: 'http://127.0.0.1:8000/v1',
      }),
    ).toBe(true);
  });

  it('requires a base URL for custom OpenAI-compatible providers', () => {
    expect(
      getDynamicModelFetchPolicy({
        provider: 'custom:local-vllm',
        apiKey: 'optional-key',
        baseUrl: '',
      }),
    ).toEqual({
      canFetch: false,
      reason: 'missing-base-url',
    });
  });
});

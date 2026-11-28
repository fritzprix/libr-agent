import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import { stableHashKeyPart } from '@/lib/ai-service/base-service-utils';
import { DEFAULT_SETTING } from '@/lib/services/settings-service';
import {
  BACKGROUND_LIST_MODELS_TIMEOUT_MS,
  buildProviderModelsSwrSegment,
  fetchDynamicProviderModels,
  fingerprintCredentialForSwrKey,
} from '../fetch-dynamic-provider-models';

const mockedListModels = vi.hoisted(() => vi.fn());

const reportListModelsFallback = vi.hoisted(() => vi.fn());

vi.mock('@/lib/backend/llm-models', () => ({
  listLlmModels: (...args: unknown[]) => mockedListModels(...args),
}));

vi.mock('@/lib/ai-service/list-models-errors', async () => {
  const actual = await vi.importActual<
    typeof import('@/lib/ai-service/list-models-errors')
  >('@/lib/ai-service/list-models-errors');
  return {
    ...actual,
    reportListModelsFallback: (...args: unknown[]) =>
      reportListModelsFallback(...args),
  };
});

// Capability detection hits the network; the tests pin the result.
const mockedGetContextWindow = vi.hoisted(
  () =>
    vi.fn<(...args: [string, unknown]) => Promise<number>>(async () => 32768),
);

vi.mock('@/lib/ai-service/model-capabilities', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('@/lib/ai-service/model-capabilities')>();
  return {
    ...actual,
    getContextWindow: (...args: unknown[]) =>
      mockedGetContextWindow(...(args as [string, unknown])),
  };
});

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

const settingsWith = (
  serviceConfigs: Partial<typeof DEFAULT_SETTING.serviceConfigs>,
) => ({
  serviceConfigs: {
    ...DEFAULT_SETTING.serviceConfigs,
    ...serviceConfigs,
  },
  customProviders: [],
});

describe('fetch-dynamic-provider-models', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedListModels.mockReset();
    reportListModelsFallback.mockReset();
  });

  it('fingerprints credentials without echoing the raw secret', () => {
    const secret = 'sk-live-super-secret';
    expect(fingerprintCredentialForSwrKey(secret)).toBe(
      stableHashKeyPart(secret),
    );
    expect(fingerprintCredentialForSwrKey(secret)).not.toBe(secret);
    expect(fingerprintCredentialForSwrKey('')).toBe('');
  });

  it('builds SWR segments with hashed api keys', () => {
    const segment = buildProviderModelsSwrSegment({
      providerId: 'openai',
      apiKey: 'sk-test',
      baseUrl: 'https://api.openai.com/v1',
      use3rdParty: false,
      customModelId: '',
    });

    expect(segment).toContain('openai|');
    expect(segment).toContain(stableHashKeyPart('sk-test'));
    expect(segment).not.toContain('sk-test');
  });

  it('fetches models through the native command and maps the Ollama format', async () => {
    mockedListModels.mockResolvedValue([
      {
        id: 'llama3.1:latest',
        name: 'llama3.1:latest',
        description: 'llama',
      },
    ]);

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settingsWith({
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      }),
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      },
    );

    expect(mockedListModels).toHaveBeenCalledWith({
      providerId: AIServiceProvider.Ollama,
      timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
    });
    expect(models['llama3.1:latest']).toMatchObject({
      id: 'llama3.1:latest',
      name: 'llama3.1:latest',
      contextWindow: 32768,
      supportReasoning: true,
      supportTools: true,
      supportStreaming: true,
      description: 'llama',
    });
    expect(reportListModelsFallback).not.toHaveBeenCalled();
  });

  it('maps the OpenAI format with the service defaults', async () => {
    mockedListModels.mockResolvedValue([
      { id: 'gpt-4o', name: 'gpt-4o', description: '' },
      { id: 'plain-model', name: 'plain-model', description: '' },
    ]);

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.OpenAI,
      settingsWith({ [AIServiceProvider.OpenAI]: { apiKey: 'sk-test' } }),
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      },
    );

    // `gpt-4o` is in the static catalog; `plain-model` falls back to the
    // service defaults (no tools, no reasoning).
    expect(models['gpt-4o'].id).toBe('gpt-4o');
    expect(models['plain-model']).toMatchObject({
      name: 'plain-model',
      contextWindow: 32768,
      supportTools: false,
      supportStreaming: true,
      description: 'plain-model',
    });
    expect(reportListModelsFallback).not.toHaveBeenCalled();
  });

  it('reports HTTP failures with notifyUser and returns empty map', async () => {
    mockedListModels.mockRejectedValue(
      new Error('HTTP 401 Unauthorized: Incorrect API key'),
    );

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settingsWith({
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      }),
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: true,
      },
    );

    expect(models).toEqual({});
    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Ollama,
        notifyUser: true,
        reason: 'api_error',
      }),
    );
  });

  it('falls back when the native call exceeds the timeout', async () => {
    mockedListModels.mockReturnValue(new Promise(() => {}));

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settingsWith({
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      }),
      { timeoutMs: 50, notifyUser: false },
    );

    expect(models).toEqual({});
    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Ollama,
        notifyUser: false,
        reason: 'api_error',
      }),
    );
  });

  it('keeps the static catalog for providers without a model list API', async () => {
    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Groq,
      settingsWith({ [AIServiceProvider.Groq]: { apiKey: 'gk-test' } }),
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: true,
      },
    );

    expect(models).toEqual({});
    expect(mockedListModels).not.toHaveBeenCalled();
    expect(reportListModelsFallback).not.toHaveBeenCalled();
  });

  it('lists Fireworks through the native command (OpenAI-compatible)', async () => {
    mockedListModels.mockResolvedValue([
      {
        id: 'accounts/fireworks/models/llama-v3p1-70b-instruct',
        name: '',
        description: '',
      },
    ]);

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Fireworks,
      settingsWith({ [AIServiceProvider.Fireworks]: { apiKey: 'fw-test' } }),
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      },
    );

    expect(mockedListModels).toHaveBeenCalledWith({
      providerId: AIServiceProvider.Fireworks,
      timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
    });
    expect(
      models['accounts/fireworks/models/llama-v3p1-70b-instruct'],
    ).toMatchObject({
      contextWindow: 32768,
      supportStreaming: true,
    });
    expect(reportListModelsFallback).not.toHaveBeenCalled();
  });

  it('gives up when a per-model capability lookup never answers', async () => {
    // The lookup runs inside the same timeout as the list call, so a hung
    // detail request must fall back instead of stalling the model fetch.
    mockedListModels.mockResolvedValue([
      { id: 'qwen2.5:7b', name: 'qwen2.5:7b', description: '' },
    ]);
    mockedGetContextWindow.mockImplementationOnce(() => new Promise(() => {}));

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settingsWith({
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      }),
      { timeoutMs: 50, notifyUser: false },
    );

    expect(models).toEqual({});
    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Ollama,
        notifyUser: false,
        reason: 'api_error',
      }),
    );
  });
});

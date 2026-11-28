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

const mockedFactory = vi.hoisted(() => ({
  getService: vi.fn(),
  invalidateService: vi.fn(),
}));

const reportListModelsFallback = vi.hoisted(() => vi.fn());

vi.mock('@/lib/ai-service/factory', async () => {
  const actual = await vi.importActual<
    typeof import('@/lib/ai-service/factory')
  >('@/lib/ai-service/factory');
  return {
    ...actual,
    AIServiceFactory: {
      ...actual.AIServiceFactory,
      getService: mockedFactory.getService,
      invalidateService: mockedFactory.invalidateService,
    },
  };
});

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

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('fetch-dynamic-provider-models', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedFactory.getService.mockReset();
    mockedFactory.invalidateService.mockReset();
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

  it('fetches and caches models on success', async () => {
    mockedFactory.getService.mockReturnValue({
      listModels: vi.fn().mockResolvedValue([
        {
          id: 'llama',
          name: 'Llama',
          contextWindow: 8192,
          supportTools: true,
          supportStreaming: true,
          supportReasoning: false,
          cost: { input: 0, output: 0 },
          description: '',
        },
      ]),
    });

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      {
        serviceConfigs: {
          ...DEFAULT_SETTING.serviceConfigs,
          [AIServiceProvider.Ollama]: {
            baseUrl: 'http://127.0.0.1:11434',
          },
        },
        customProviders: [],
      },
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      },
    );

    expect(models.llama?.name).toBe('Llama');
    expect(reportListModelsFallback).not.toHaveBeenCalled();
  });

  it('reports failures with notifyUser and returns empty map', async () => {
    mockedFactory.getService.mockReturnValue({
      listModels: vi.fn().mockRejectedValue(new Error('connection refused')),
    });

    const models = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      {
        serviceConfigs: {
          ...DEFAULT_SETTING.serviceConfigs,
          [AIServiceProvider.Ollama]: {
            baseUrl: 'http://127.0.0.1:11434',
          },
        },
        customProviders: [],
      },
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

  it('a second refresh returns the changed list (#3092 G1.1)', async () => {
    // Mimics the real cache chain: the factory hands out one service instance
    // until it is invalidated, and the instance keeps the list it fetched.
    let currentList: unknown[] = [
      {
        id: 'alpha',
        name: 'Alpha',
        contextWindow: 1000,
        supportTools: true,
        supportStreaming: true,
        supportReasoning: false,
        cost: { input: 0, output: 0 },
        description: '',
      },
    ];
    let cachedService: { listModels: () => Promise<unknown[]> } | null = null;
    mockedFactory.getService.mockImplementation(() => {
      if (!cachedService) {
        const snapshot = currentList;
        cachedService = { listModels: async () => snapshot };
      }
      return cachedService;
    });
    mockedFactory.invalidateService.mockImplementation(() => {
      cachedService = null;
    });

    const settings = {
      serviceConfigs: {
        ...DEFAULT_SETTING.serviceConfigs,
        [AIServiceProvider.Ollama]: {
          baseUrl: 'http://127.0.0.1:11434',
        },
      },
      customProviders: [],
    };

    const first = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settings,
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: true,
        refresh: true,
      },
    );
    expect(Object.keys(first)).toEqual(['alpha']);

    // The provider adds a model between the two refreshes.
    currentList = [
      ...currentList,
      {
        id: 'beta',
        name: 'Beta',
        contextWindow: 2000,
        supportTools: true,
        supportStreaming: true,
        supportReasoning: false,
        cost: { input: 0, output: 0 },
        description: '',
      },
    ];

    const second = await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      settings,
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: true,
        refresh: true,
      },
    );
    expect(Object.keys(second).sort()).toEqual(['alpha', 'beta']);
  });

  it('a background fetch does not invalidate the cached service', async () => {
    mockedFactory.getService.mockReturnValue({
      listModels: vi.fn().mockResolvedValue([]),
    });

    await fetchDynamicProviderModels(
      AIServiceProvider.Ollama,
      {
        serviceConfigs: {
          ...DEFAULT_SETTING.serviceConfigs,
          [AIServiceProvider.Ollama]: {
            baseUrl: 'http://127.0.0.1:11434',
          },
        },
        customProviders: [],
      },
      {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      },
    );

    expect(mockedFactory.invalidateService).not.toHaveBeenCalled();
  });
});

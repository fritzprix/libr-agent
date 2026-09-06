import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import { stableHashKeyPart } from '@/lib/ai-service/base-service-utils';
import { buildProviderModelsSwrSegment } from '@/lib/ai-service/fetch-dynamic-provider-models';
import { useGroupedAgentModels } from '../useGroupedAgentModels';

const mockState = vi.hoisted(() => ({
  mutate: vi.fn().mockResolvedValue({}),
  data: {} as Record<string, Record<string, unknown>>,
  isValidating: false,
  swrKey: undefined as unknown,
  fetcher: undefined as
    | ((key: readonly ['grouped-models', string]) => Promise<unknown>)
    | undefined,
  serviceConfigs: {} as Record<string, Record<string, unknown>>,
  customProviders: [] as Array<{
    id: string;
    name: string;
    baseUrl: string;
    apiKey?: string;
  }>,
}));

const mockedFactory = vi.hoisted(() => ({
  getService: vi.fn(),
  invalidateService: vi.fn(),
}));

const reportListModelsFallback = vi.hoisted(() => vi.fn());

vi.mock('swr', () => ({
  default: vi.fn(
    (
      key: unknown,
      fetcher?: (key: readonly ['grouped-models', string]) => Promise<unknown>,
    ) => {
      mockState.swrKey = key;
      mockState.fetcher = fetcher;
      return {
        data: mockState.data,
        mutate: mockState.mutate,
        isValidating: mockState.isValidating,
      };
    },
  ),
}));

vi.mock('@/lib/ai-service', async () => {
  const actual = await vi.importActual<typeof import('@/lib/ai-service')>(
    '@/lib/ai-service',
  );

  return {
    ...actual,
    AIServiceFactory: {
      ...actual.AIServiceFactory,
      getService: mockedFactory.getService,
      invalidateService: mockedFactory.invalidateService,
    },
  };
});

// fetchDynamicProviderModels imports the factory module directly.
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

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      serviceConfigs: mockState.serviceConfigs,
      customProviders: mockState.customProviders,
    },
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
  },
}));

describe('useGroupedAgentModels', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mockState.data = {};
    mockState.isValidating = false;
    mockState.swrKey = undefined;
    mockState.fetcher = undefined;
    mockState.serviceConfigs = {
      [AIServiceProvider.Ollama]: {
        baseUrl: 'http://127.0.0.1:11434',
      },
    };
    mockState.customProviders = [];
    mockedFactory.getService.mockReset();
    mockedFactory.invalidateService.mockReset();
    reportListModelsFallback.mockReset();
    mockState.mutate.mockReset();
    mockState.mutate.mockResolvedValue({});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('builds a grouped SWR key with fingerprinted credentials', () => {
    renderHook(() =>
      useGroupedAgentModels({ currentProvider: AIServiceProvider.Ollama }),
    );

    const segment = buildProviderModelsSwrSegment({
      providerId: 'ollama',
      apiKey: '',
      baseUrl: 'http://127.0.0.1:11434',
      use3rdParty: false,
      customModelId: '',
    });

    expect(mockState.swrKey).toEqual(['grouped-models', segment]);
  });

  it('debounces draft credential changes before updating the SWR key', () => {
    const { rerender } = renderHook(
      ({
        serviceConfigs,
      }: {
        serviceConfigs: Record<string, Record<string, unknown>>;
      }) =>
        useGroupedAgentModels({
          currentProvider: AIServiceProvider.OpenAI,
          serviceConfigs: serviceConfigs as never,
        }),
      {
        initialProps: {
          serviceConfigs: {
            [AIServiceProvider.OpenAI]: {
              apiKey: 'key-a',
              baseUrl: 'https://api.example.com/v1',
            },
          },
        },
      },
    );

    const hashA = stableHashKeyPart('key-a');
    const hashAB = stableHashKeyPart('key-ab');

    expect(String(mockState.swrKey)).toContain(hashA);
    expect(String(mockState.swrKey)).not.toContain('key-a');

    rerender({
      serviceConfigs: {
        [AIServiceProvider.OpenAI]: {
          apiKey: 'key-ab',
          baseUrl: 'https://api.example.com/v1',
        },
      },
    });

    expect(String(mockState.swrKey)).toContain(hashA);
    expect(String(mockState.swrKey)).not.toContain(hashAB);

    act(() => {
      vi.advanceTimersByTime(800);
    });

    expect(String(mockState.swrKey)).toContain(hashAB);
    expect(String(mockState.swrKey)).not.toContain('key-ab');
  });

  it('refreshModels notifies the user on failure and skips SWR revalidate', async () => {
    mockedFactory.getService.mockReturnValue({
      listModels: vi.fn().mockRejectedValue(new Error('Operation timed out')),
    });

    const { result } = renderHook(() =>
      useGroupedAgentModels({ currentProvider: AIServiceProvider.Ollama }),
    );

    await act(async () => {
      await result.current.refreshModels();
    });

    expect(mockedFactory.invalidateService).toHaveBeenCalled();
    expect(mockState.mutate).toHaveBeenCalledWith(expect.any(Function), {
      revalidate: false,
    });

    const updater = mockState.mutate.mock.calls[0]?.[0] as (
      current: Record<string, unknown>,
    ) => Promise<Record<string, unknown>>;

    await act(async () => {
      await updater({});
    });

    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Ollama,
        reason: 'api_error',
        notifyUser: true,
      }),
    );
  });

  it('background fetcher reports failures without notifyUser', async () => {
    mockedFactory.getService.mockReturnValue({
      listModels: vi.fn().mockRejectedValue(new Error('connection refused')),
    });

    renderHook(() =>
      useGroupedAgentModels({ currentProvider: AIServiceProvider.Ollama }),
    );

    expect(mockState.fetcher).toEqual(expect.any(Function));

    const segment = buildProviderModelsSwrSegment({
      providerId: 'ollama',
      apiKey: '',
      baseUrl: 'http://127.0.0.1:11434',
      use3rdParty: false,
      customModelId: '',
    });

    await act(async () => {
      await mockState.fetcher?.(['grouped-models', segment]);
    });

    expect(reportListModelsFallback).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: AIServiceProvider.Ollama,
        notifyUser: false,
      }),
    );
  });
});

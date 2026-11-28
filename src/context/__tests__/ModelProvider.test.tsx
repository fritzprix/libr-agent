import { act, renderHook } from '@testing-library/react';
import { createElement, type FC, type PropsWithChildren } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ModelOptionsProvider, useModelOptions } from '../ModelProvider';

const mockState = vi.hoisted(() => ({
  mutate: vi.fn().mockResolvedValue({}),
  fetcher: undefined as
    | ((key: readonly ['models', string]) => Promise<unknown>)
    | undefined,
  settings: {
    serviceConfigs: {
      ollama: { baseUrl: 'http://127.0.0.1:11434' },
    } as Record<string, Record<string, unknown>>,
    customProviders: [] as never[],
    preferredModel: { model: 'llama3', provider: 'ollama' },
    fallbackModel: undefined,
  },
}));

const mockedFetch = vi.hoisted(() => ({
  fetchDynamicProviderModels: vi.fn(),
}));

vi.mock('swr', () => ({
  default: vi.fn(
    (
      _key: unknown,
      fetcher?: (key: readonly ['models', string]) => Promise<unknown>,
    ) => {
      mockState.fetcher = fetcher;
      return {
        data: {},
        mutate: mockState.mutate,
        isValidating: false,
      };
    },
  ),
}));

// Keep the real segment builders and timeout constants; only spy on the
// fetch so the options each caller passes are observable.
vi.mock('@/lib/ai-service/fetch-dynamic-provider-models', async () => {
  const actual = await vi.importActual<
    typeof import('@/lib/ai-service/fetch-dynamic-provider-models')
  >('@/lib/ai-service/fetch-dynamic-provider-models');
  return {
    ...actual,
    fetchDynamicProviderModels: mockedFetch.fetchDynamicProviderModels,
  };
});

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: mockState.settings,
    update: vi.fn(),
    isLoading: false,
  }),
}));

vi.mock('@/lib/ai-service/model-cache-storage', () => ({
  getStoredModelCache: () => ({}),
}));

vi.mock('@/lib/ai-service/last-selected-model-storage', () => ({
  setLastSelectedModel: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

function renderModelOptions() {
  const wrapper: FC<PropsWithChildren> = ({ children }) =>
    createElement(ModelOptionsProvider, null, children);
  return renderHook(() => useModelOptions(), { wrapper });
}

describe('ModelOptionsProvider model refresh', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState.fetcher = undefined;
    mockState.mutate.mockResolvedValue({});
    mockedFetch.fetchDynamicProviderModels.mockResolvedValue({});
  });

  it('refreshModels asks for a fresh list with the refresh option', async () => {
    const { result } = renderModelOptions();

    await act(async () => {
      await result.current.refreshModels();
    });

    // The factory caches each provider's service (and its model list) for an
    // hour; only `refresh: true` drops the cached service. Deleting the flag
    // in the caller must fail this test.
    expect(mockedFetch.fetchDynamicProviderModels).toHaveBeenCalledWith(
      'ollama',
      expect.anything(),
      expect.objectContaining({ refresh: true, notifyUser: true }),
    );
    expect(mockState.mutate).toHaveBeenCalledWith(expect.anything(), {
      revalidate: false,
    });
  });

  it('background fetcher does not ask for a refresh', async () => {
    renderModelOptions();

    expect(mockState.fetcher).toEqual(expect.any(Function));

    await act(async () => {
      await mockState.fetcher?.(['models', 'ollama|segment']);
    });

    expect(mockedFetch.fetchDynamicProviderModels).toHaveBeenCalledWith(
      'ollama',
      expect.anything(),
      expect.objectContaining({ notifyUser: false }),
    );
    const options =
      mockedFetch.fetchDynamicProviderModels.mock.calls[0]?.[2] ?? {};
    expect(options.refresh).toBeUndefined();
  });
});

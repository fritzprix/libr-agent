import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import { useAgentModels } from '../useAgentModels';

const mockState = vi.hoisted(() => ({
  mutate: vi.fn().mockResolvedValue({}),
  data: {},
  isValidating: false,
  swrKey: undefined as unknown,
  serviceConfigs: {} as Record<string, Record<string, unknown>>,
}));

const mockedFactory = vi.hoisted(() => ({
  invalidateService: vi.fn(),
}));

vi.mock('swr', () => ({
  default: vi.fn((key: unknown) => {
    mockState.swrKey = key;
    return {
      data: mockState.data,
      mutate: mockState.mutate,
      isValidating: mockState.isValidating,
    };
  }),
}));

vi.mock('@/lib/ai-service', async () => {
  const actual = await vi.importActual<typeof import('@/lib/ai-service')>(
    '@/lib/ai-service',
  );

  return {
    ...actual,
    AIServiceFactory: {
      ...actual.AIServiceFactory,
      invalidateService: mockedFactory.invalidateService,
    },
  };
});

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      serviceConfigs: mockState.serviceConfigs,
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

describe('useAgentModels', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState.data = {};
    mockState.isValidating = false;
    mockState.swrKey = undefined;
    mockState.serviceConfigs = {};
    mockedFactory.invalidateService.mockReset();
  });

  it('does not call mutate when dynamic refresh is unavailable', async () => {
    const { result } = renderHook(() =>
      useAgentModels(AIServiceProvider.Anthropic),
    );

    expect(result.current.canRefresh).toBe(false);
    expect(result.current.refreshBlockedReason).toBe('missing-api-key');
    expect(mockState.swrKey).toBeNull();

    await act(async () => {
      await result.current.refreshModels();
    });

    expect(mockState.mutate).not.toHaveBeenCalled();
  });

  it('revalidates the active SWR entry when dynamic refresh is available', async () => {
    const { result } = renderHook(() =>
      useAgentModels(AIServiceProvider.Ollama),
    );

    expect(result.current.canRefresh).toBe(true);
    expect(result.current.refreshBlockedReason).toBe('allowed');
    expect(mockState.swrKey).toEqual([
      'local-models',
      'ollama',
      '',
      '',
      'first-party',
      '',
    ]);

    await act(async () => {
      await result.current.refreshModels();
    });

    expect(mockState.mutate).toHaveBeenCalledTimes(1);
    expect(mockedFactory.invalidateService).toHaveBeenCalledWith(
      AIServiceProvider.Ollama,
      '',
      {},
    );
  });

  it('uses the settings draft override for refresh keys and invalidation', async () => {
    const serviceConfigOverride = {
      apiKey: 'draft-key',
      baseUrl: 'https://draft.example.com/v1',
      use3rdParty: true,
      customModelId: '',
    };

    const { result } = renderHook(() =>
      useAgentModels(AIServiceProvider.OpenAI, serviceConfigOverride),
    );

    expect(mockState.swrKey).toEqual([
      'local-models',
      'openai',
      'draft-key',
      'https://draft.example.com/v1',
      'use-3rd-party',
      '',
    ]);

    await act(async () => {
      await result.current.refreshModels();
    });

    expect(mockedFactory.invalidateService).toHaveBeenCalledWith(
      AIServiceProvider.OpenAI,
      'draft-key',
      serviceConfigOverride,
    );
  });
});

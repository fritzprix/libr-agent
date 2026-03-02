import { describe, it, expect, vi, beforeEach } from 'vitest';
import { OpenRouterService } from '../openrouter';
import { AIServiceProvider } from '../types';

// Mock the openrouter-metadata module so tests don't hit the real network
vi.mock('../openrouter-metadata', () => ({
  fetchOpenRouterModels: vi.fn(),
  clearMetadataCache: vi.fn(),
}));

// Mock retry-utils to execute immediately (no delays in tests)
vi.mock('../../retry-utils', () => ({
  withRetry: vi.fn().mockImplementation((fn: () => unknown) => fn()),
  withTimeout: vi.fn().mockImplementation((promise: Promise<unknown>) => promise),
}));

// Mock the OpenAI SDK (required by OpenAIService constructor)
vi.mock('openai', () => ({
  default: vi.fn().mockImplementation(() => ({
    models: {
      list: vi.fn().mockRejectedValue(new Error('mocked: no real HTTP calls')),
    },
  })),
}));

// Mock the llm-config-manager (used by BaseAIService.listModels fallback)
vi.mock('../../llm-config-manager', () => ({
  llmConfigManager: {
    getModelsForProvider: vi.fn().mockReturnValue({
      'anthropic/claude-3-5-sonnet': {
        id: 'anthropic/claude-3-5-sonnet',
        name: 'Claude 3.5 Sonnet (via OpenRouter)',
        contextWindow: 200000,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: { input: 0.003, output: 0.015 },
        description: 'Static fallback model',
      },
    }),
    getModel: vi.fn().mockReturnValue(null),
  },
}));

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

// Minimal OpenRouterModel fixture
const makeOpenRouterModel = (overrides: Partial<{
  id: string;
  name: string;
  description: string;
  context_length: number;
  supported_parameters: string[];
  pricing: { prompt: string; completion: string; internal_reasoning?: string };
}> = {}) => ({
  id: 'openai/gpt-4o',
  name: 'GPT-4o',
  description: 'GPT-4o model',
  context_length: 128000,
  architecture: { input_modalities: ['text', 'image'], output_modalities: ['text'] },
  supported_parameters: ['tools', 'stream'],
  pricing: { prompt: '0.000005', completion: '0.000015' },
  top_provider: { max_completion_tokens: 4096 },
  ...overrides,
});

describe('OpenRouterService', () => {
  let service: OpenRouterService;

  beforeEach(async () => {
    vi.clearAllMocks();
    service = new OpenRouterService('sk-or-test-key');
  });

  describe('getProvider()', () => {
    it('returns AIServiceProvider.OpenRouter', () => {
      expect(service.getProvider()).toBe(AIServiceProvider.OpenRouter);
    });
  });

  describe('listModels()', () => {
    it('uses fetchOpenRouterModels — not the OpenAI client', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      const mockFetch = vi.mocked(fetchOpenRouterModels);
      mockFetch.mockResolvedValue(new Map([['openai/gpt-4o', makeOpenRouterModel()]]));

      await service.listModels();

      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    it('maps OpenRouter model fields to ModelInfo correctly', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      vi.mocked(fetchOpenRouterModels).mockResolvedValue(
        new Map([
          ['openai/gpt-4o', makeOpenRouterModel({
            id: 'openai/gpt-4o',
            name: 'GPT-4o',
            context_length: 128000,
            supported_parameters: ['tools', 'stream'],
            pricing: { prompt: '0.000005', completion: '0.000015' },
          })],
        ]),
      );

      const [model] = await service.listModels();

      expect(model.id).toBe('openai/gpt-4o');
      expect(model.name).toBe('GPT-4o');
      expect(model.contextWindow).toBe(128000);
      expect(model.supportTools).toBe(true);
      expect(model.supportStreaming).toBe(true);
      // Pricing: prompt 0.000005 * 1_000_000 = 5 (per million tokens)
      expect(model.cost.input).toBeCloseTo(5);
      expect(model.cost.output).toBeCloseTo(15);
    });

    it('detects reasoning support via supported_parameters', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      vi.mocked(fetchOpenRouterModels).mockResolvedValue(
        new Map([
          ['openai/o3', makeOpenRouterModel({
            id: 'openai/o3',
            supported_parameters: ['reasoning', 'tools'],
            pricing: { prompt: '0.000010', completion: '0.000040' },
          })],
        ]),
      );

      const [model] = await service.listModels();
      expect(model.supportReasoning).toBe(true);
    });

    it('detects reasoning support via internal_reasoning pricing', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      vi.mocked(fetchOpenRouterModels).mockResolvedValue(
        new Map([
          ['anthropic/claude-3-5-sonnet', makeOpenRouterModel({
            id: 'anthropic/claude-3-5-sonnet',
            supported_parameters: ['tools'],
            pricing: {
              prompt: '0.000003',
              completion: '0.000015',
              internal_reasoning: '0.000003',
            },
          })],
        ]),
      );

      const [model] = await service.listModels();
      expect(model.supportReasoning).toBe(true);
    });

    it('returns all models from the metadata map (400+ in production)', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      const bigMap = new Map(
        Array.from({ length: 50 }, (_, i) => [
          `provider/model-${i}`,
          makeOpenRouterModel({ id: `provider/model-${i}`, name: `Model ${i}` }),
        ]),
      );
      vi.mocked(fetchOpenRouterModels).mockResolvedValue(bigMap);

      const result = await service.listModels();
      expect(result).toHaveLength(50);
    });

    it('falls back to static config (super.listModels) when fetchOpenRouterModels throws', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      vi.mocked(fetchOpenRouterModels).mockRejectedValue(new Error('Network error'));

      const result = await service.listModels();

      // Should not throw; falls back to static config
      expect(result).toBeInstanceOf(Array);
      expect(result.length).toBeGreaterThan(0);
    });

    it('handles missing context_length gracefully (defaults to 4096)', async () => {
      const { fetchOpenRouterModels } = await import('../openrouter-metadata');
      vi.mocked(fetchOpenRouterModels).mockResolvedValue(
        new Map([
          ['some/model', makeOpenRouterModel({ context_length: undefined as unknown as number })],
        ]),
      );

      const [model] = await service.listModels();
      expect(model.contextWindow).toBe(4096);
    });
  });
});

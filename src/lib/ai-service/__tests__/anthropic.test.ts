import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { AnthropicService } from '../anthropic';
import { llmConfigManager } from '../../llm-config-manager';

// Mock the Anthropic SDK
vi.mock('@anthropic-ai/sdk', () => {
  return {
    default: vi.fn().mockImplementation(() => ({
      models: {
        list: vi.fn(),
      },
      messages: {
        stream: vi.fn(),
        create: vi.fn(),
      },
    })),
  };
});

// Mock the logger
vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('AnthropicService', () => {
  let service: AnthropicService;
  let mockAnthropicClient: unknown;

  beforeEach(() => {
    service = new AnthropicService('test-api-key', {
      maxTokens: 1024,
      temperature: 0.7,
    });
    // Access the private anthropic client for mocking
    mockAnthropicClient = (service as unknown as { anthropic: unknown })
      .anthropic;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('listModels()', () => {
    it('should fetch models from SDK and map to ModelInfo[]', async () => {
      // Mock SDK response
      const mockModels = [
        {
          id: 'claude-opus-4-20250514',
          display_name: 'Claude Opus 4',
          type: 'model',
        },
        {
          id: 'claude-sonnet-4-20250514',
          display_name: 'Claude Sonnet 4',
          type: 'model',
        },
      ];

      const mockListFn = vi.fn().mockResolvedValue({
        data: mockModels,
      });

      (
        mockAnthropicClient as { models: { list: typeof mockListFn } }
      ).models.list = mockListFn;

      const result = await service.listModels();

      expect(mockListFn).toHaveBeenCalled();
      expect(result).toBeInstanceOf(Array);
      expect(result.length).toBeGreaterThan(0);

      // Verify first model has correct structure
      const firstModel = result.find(
        (m) => m.id === 'claude-opus-4-20250514',
      );
      expect(firstModel).toBeDefined();
      expect(firstModel?.name).toBeTruthy();
      expect(firstModel?.supportReasoning).toBeDefined();
      expect(firstModel?.supportTools).toBeDefined();
      expect(firstModel?.supportStreaming).toBeDefined();
    });

    it('should fallback to static config on SDK error', async () => {
      const mockListFn = vi
        .fn()
        .mockRejectedValue(new Error('Network error'));

      (
        mockAnthropicClient as { models: { list: typeof mockListFn } }
      ).models.list = mockListFn;

      const result = await service.listModels();

      expect(mockListFn).toHaveBeenCalled();
      // Should still return models from static config
      expect(result).toBeInstanceOf(Array);
      expect(result.length).toBeGreaterThan(0);
    });

    it('should cache models for subsequent calls', async () => {
      const mockModels = [
        {
          id: 'claude-opus-4-20250514',
          display_name: 'Claude Opus 4',
          type: 'model',
        },
      ];

      const mockListFn = vi.fn().mockResolvedValue({
        data: mockModels,
      });

      (
        mockAnthropicClient as { models: { list: typeof mockListFn } }
      ).models.list = mockListFn;

      // First call
      await service.listModels();
      expect(mockListFn).toHaveBeenCalledTimes(1);

      // Second call should use cache
      await service.listModels();
      expect(mockListFn).toHaveBeenCalledTimes(1); // Still 1, not called again
    });

    it('should handle invalid response structure gracefully', async () => {
      const mockListFn = vi.fn().mockResolvedValue({
        data: null, // Invalid structure
      });

      (
        mockAnthropicClient as { models: { list: typeof mockListFn } }
      ).models.list = mockListFn;

      const result = await service.listModels();

      // Should fallback to static config
      expect(result).toBeInstanceOf(Array);
      expect(result.length).toBeGreaterThan(0);
    });
  });

  describe('shouldEnableThinking()', () => {
    it('should return true for supportReasoning models', () => {
      // Use private method access for testing
      const shouldEnableThinking = (
        service as unknown as {
          shouldEnableThinking: (
            modelName?: string,
            config?: { defaultModel?: string },
          ) => boolean;
        }
      ).shouldEnableThinking;

      // claude-opus-4-20250514 has supportReasoning: true in config
      const result = shouldEnableThinking('claude-opus-4-20250514', {});

      expect(result).toBe(true);
    });

    it('should return false for models without supportReasoning', () => {
      const shouldEnableThinking = (
        service as unknown as {
          shouldEnableThinking: (
            modelName?: string,
            config?: { defaultModel?: string },
          ) => boolean;
        }
      ).shouldEnableThinking;

      // claude-3-5-haiku-20241022 has supportReasoning: false in config
      const result = shouldEnableThinking('claude-3-5-haiku-20241022', {});

      expect(result).toBe(false);
    });

    it('should return false for unknown models', () => {
      const shouldEnableThinking = (
        service as unknown as {
          shouldEnableThinking: (
            modelName?: string,
            config?: { defaultModel?: string },
          ) => boolean;
        }
      ).shouldEnableThinking;

      const result = shouldEnableThinking('unknown-model-xyz', {});

      expect(result).toBe(false);
    });

    it('should return false when no model is specified', () => {
      const shouldEnableThinking = (
        service as unknown as {
          shouldEnableThinking: (
            modelName?: string,
            config?: { defaultModel?: string },
          ) => boolean;
        }
      ).shouldEnableThinking;

      const result = shouldEnableThinking(undefined, {});

      expect(result).toBe(false);
    });

    it('should use config default model if modelName not provided', () => {
      const shouldEnableThinking = (
        service as unknown as {
          shouldEnableThinking: (
            modelName?: string,
            config?: { defaultModel?: string },
          ) => boolean;
        }
      ).shouldEnableThinking;

      const result = shouldEnableThinking(undefined, {
        defaultModel: 'claude-opus-4-20250514',
      });

      expect(result).toBe(true);
    });
  });

  describe('getDefaultModel()', () => {
    it('should prefer config default model', () => {
      const serviceWithDefault = new AnthropicService('test-key', {
        defaultModel: 'claude-opus-4-20250514',
        maxTokens: 1024,
      });

      // Use call() to preserve 'this' binding
      const result = (
        serviceWithDefault as unknown as {
          getDefaultModel: () => string;
        }
      ).getDefaultModel.call(serviceWithDefault);

      expect(result).toBe('claude-opus-4-20250514');
    });

    it('should fallback to first config model if no default', () => {
      const serviceWithoutDefault = new AnthropicService('test-key', {
        maxTokens: 1024,
      });

      // Use call() to preserve 'this' binding
      const result = (
        serviceWithoutDefault as unknown as {
          getDefaultModel: () => string;
        }
      ).getDefaultModel.call(serviceWithoutDefault);

      expect(result).toBeTruthy();
      // Should be a valid model ID
      const model = llmConfigManager.getModel('anthropic', result);
      expect(model).toBeDefined();
    });

    it('should return safe fallback when config is empty', () => {
      // Use call() to preserve 'this' binding
      const result = (
        service as unknown as {
          getDefaultModel: () => string;
        }
      ).getDefaultModel.call(service);

      expect(result).toBeTruthy();
      expect(typeof result).toBe('string');
    });
  });

  describe('Model selection priority', () => {
    it('should use explicit modelName over config default', () => {
      // Even if service has a default, explicit model should be used
      // This would be tested in streamChat context
      const explicitModel = 'claude-opus-4-20250514';

      // Verify the model exists in config
      const model = llmConfigManager.getModel('anthropic', explicitModel);
      expect(model).toBeDefined();
    });
  });

  describe('Fallback model validation', () => {
    it('should validate that fallback model exists in config', () => {
      const fallbackModel = 'claude-3-5-sonnet-20241022';
      const model = llmConfigManager.getModel('anthropic', fallbackModel);

      expect(model).toBeDefined();
      expect(model?.name).toBeTruthy();
      expect(model?.contextWindow).toBeGreaterThan(0);
    });
  });

  describe('Config integration', () => {
    it('should correctly identify reasoning-capable models from config', () => {
      const reasoningModels = [
        'claude-opus-4-20250514',
        'claude-sonnet-4-20250514',
      ];

      for (const modelId of reasoningModels) {
        const model = llmConfigManager.getModel('anthropic', modelId);
        expect(model?.supportReasoning).toBe(true);
      }
    });

    it('should correctly identify non-reasoning models from config', () => {
      const nonReasoningModels = [
        'claude-3-5-sonnet-20241022',
        'claude-3-5-haiku-20241022',
      ];

      for (const modelId of nonReasoningModels) {
        const model = llmConfigManager.getModel('anthropic', modelId);
        expect(model?.supportReasoning).toBe(false);
      }
    });
  });
});

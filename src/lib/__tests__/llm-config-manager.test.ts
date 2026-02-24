import { describe, it, expect, vi, beforeEach } from 'vitest';
import { LLMConfigManager } from '../llm-config-manager';

// Mock the config
vi.mock('../../config/llm-config.json', () => ({
  default: {
    providers: {
      openai: {
        name: 'OpenAI',
        apiKeyEnvVar: 'OPENAI_API_KEY',
        baseUrl: 'https://api.openai.com/v1',
        models: {
          'gpt-4o': {
            name: 'GPT-4o',
            contextWindow: 128000,
            supportReasoning: false,
            supportTools: true,
            supportStreaming: true,
            cost: { input: 0.0025, output: 0.01 },
            description: 'Most capable GPT-4 model',
          },
          'gpt-3.5-turbo': {
            name: 'GPT-3.5 Turbo',
            contextWindow: 16385,
            supportReasoning: false,
            supportTools: true,
            supportStreaming: true,
            cost: { input: 0.0005, output: 0.0015 },
            description: 'Fast and cheap',
          },
        },
      },
      anthropic: {
        name: 'Anthropic',
        apiKeyEnvVar: 'ANTHROPIC_API_KEY',
        baseUrl: 'https://api.anthropic.com',
        models: {
          'claude-3-opus': {
            name: 'Claude 3 Opus',
            contextWindow: 200000,
            supportReasoning: true,
            supportTools: true,
            supportStreaming: true,
            cost: { input: 0.015, output: 0.075 },
            description: 'Most intelligent model',
          },
        },
      },
      gemini: {
        name: 'Google',
        apiKeyEnvVar: 'GOOGLE_API_KEY',
        baseUrl: 'https://generativelanguage.googleapis.com/v1beta',
        models: {
          'gemini-1.5-pro': {
            name: 'Gemini 1.5 Pro',
            contextWindow: 1000000,
            supportReasoning: true,
            supportTools: true,
            supportStreaming: true,
            cost: { input: 0.0035, output: 0.0105 },
            description: 'Long context window',
          },
        },
      },
      groq: {
          name: 'Groq',
          apiKeyEnvVar: 'GROQ_API_KEY',
          baseUrl: 'https://api.groq.com/openai/v1',
          models: {
            'llama3-8b-8192': {
                name: 'Llama 3 8B',
                contextWindow: 8192,
                supportReasoning: false,
                supportTools: true,
                supportStreaming: true,
                cost: { input: 0.00005, output: 0.0001 },
                description: 'Fast open source model'
            }
          }
      }
    },
  },
}));

describe('LLMConfigManager', () => {
  let manager: LLMConfigManager;

  beforeEach(() => {
    manager = new LLMConfigManager();
  });

  describe('Data Access', () => {
    it('should get all providers', () => {
      const providers = manager.getProviders();
      expect(Object.keys(providers)).toHaveLength(4);
      expect(providers).toHaveProperty('openai');
      expect(providers).toHaveProperty('anthropic');
      expect(providers).toHaveProperty('gemini');
      expect(providers).toHaveProperty('groq');
    });

    it('should get a specific provider by ID', () => {
      const provider = manager.getProvider('openai');
      expect(provider).toBeDefined();
      expect(provider?.name).toBe('OpenAI');
    });

    it('should return null for non-existent provider', () => {
      const provider = manager.getProvider('non-existent');
      expect(provider).toBeNull();
    });

    it('should get a specific model', () => {
      const model = manager.getModel('openai', 'gpt-4o');
      expect(model).toBeDefined();
      expect(model?.name).toBe('GPT-4o');
    });

    it('should return null for non-existent model', () => {
      const model = manager.getModel('openai', 'non-existent-model');
      expect(model).toBeNull();
    });

    it('should return null for model in non-existent provider', () => {
        const model = manager.getModel('non-existent', 'gpt-4o');
        expect(model).toBeNull();
    });

    it('should get all models for a provider', () => {
      const models = manager.getModelsForProvider('openai');
      expect(models).toBeDefined();
      expect(Object.keys(models!)).toHaveLength(2);
      expect(models).toHaveProperty('gpt-4o');
      expect(models).toHaveProperty('gpt-3.5-turbo');
    });

    it('should return null models for non-existent provider', () => {
        const models = manager.getModelsForProvider('non-existent');
        expect(models).toBeNull();
    });

    it('should get all models flattened', () => {
      const allModels = manager.getAllModels();
      // 2 openai + 1 anthropic + 1 gemini + 1 groq = 5 models
      expect(allModels).toHaveLength(5);
      const gpt4o = allModels.find(m => m.modelId === 'gpt-4o' && m.providerId === 'openai');
      expect(gpt4o).toBeDefined();
    });

    it('should get all service IDs', () => {
      const ids = manager.getServiceIds();
      expect(ids).toContain('openai');
      expect(ids).toContain('anthropic');
      expect(ids).toContain('gemini');
      expect(ids).toContain('groq');
    });
  });

  describe('Logic and Filtering', () => {
    it('should generate LangChain model IDs correctly', () => {
      expect(manager.getLangchainModelId('openai', 'gpt-4o')).toBe('openai:gpt-4o');
      expect(manager.getLangchainModelId('anthropic', 'claude-3-opus')).toBe('anthropic:claude-3-opus');
      expect(manager.getLangchainModelId('groq', 'llama3-8b-8192')).toBe('groq:llama3-8b-8192');
      // Should work after fix
      expect(manager.getLangchainModelId('gemini', 'gemini-1.5-pro')).toBe('google-genai:gemini-1.5-pro');
    });

    it('should throw for unknown provider in getLangchainModelId', () => {
        expect(() => manager.getLangchainModelId('unknown', 'model')).toThrow('Unknown provider: unknown');
    });

    it('should filter models with tools', () => {
        const toolsModels = manager.getModelsWithTools();
        expect(toolsModels).toHaveLength(5);
    });

    it('should filter models with reasoning', () => {
        const reasoningModels = manager.getModelsWithReasoning();
        expect(reasoningModels).toHaveLength(2);
        expect(reasoningModels.some(m => m.modelId === 'claude-3-opus')).toBe(true);
        expect(reasoningModels.some(m => m.modelId === 'gemini-1.5-pro')).toBe(true);
    });

    it('should filter models by cost range', () => {
        const cheapModels = manager.getModelsByCostRange(0.001, 1.0);
        expect(cheapModels).toHaveLength(2);
        expect(cheapModels.some(m => m.modelId === 'gpt-3.5-turbo')).toBe(true);
        expect(cheapModels.some(m => m.modelId === 'llama3-8b-8192')).toBe(true);
    });
  });

  describe('Validation and Recommendation', () => {
      it('should validate valid service config', () => {
          const isValid = manager.validateServiceConfig({
              provider: 'openai',
              model: 'gpt-4o',
              temperature: 0.7,
              maxTokens: 1000,
              topP: 1,
              frequencyPenalty: 0,
              presencePenalty: 0
          });
          expect(isValid).toBe(true);
      });

      it('should invalidate config with non-existent provider', () => {
        const isValid = manager.validateServiceConfig({
            provider: 'unknown',
            model: 'gpt-4o',
            temperature: 0.7,
            maxTokens: 1000,
            topP: 1,
            frequencyPenalty: 0,
            presencePenalty: 0
        });
        expect(isValid).toBe(false);
      });

      it('should invalidate config with non-existent model', () => {
        const isValid = manager.validateServiceConfig({
            provider: 'openai',
            model: 'unknown-model',
            temperature: 0.7,
            maxTokens: 1000,
            topP: 1,
            frequencyPenalty: 0,
            presencePenalty: 0
        });
        expect(isValid).toBe(false);
      });

      it('should recommend a model based on reasoning requirement', () => {
          const result = manager.recommendModel({ needsReasoning: true });
          expect(result).toBeDefined();
          expect(result?.providerId).toBe('gemini');
          expect(result?.modelId).toBe('gemini-1.5-pro');
      });

      it('should recommend a model based on speed (lowest cost)', () => {
        const result = manager.recommendModel({ preferSpeed: true });
        expect(result?.providerId).toBe('groq');
        expect(result?.modelId).toBe('llama3-8b-8192');
      });

      it('should recommend a model based on max cost', () => {
          const result = manager.recommendModel({ maxCost: 0.002 });
          expect(result?.providerId).toBe('openai');
          expect(result?.modelId).toBe('gpt-3.5-turbo');
      });

      it('should return null if no model matches requirements', () => {
          const result = manager.recommendModel({ maxCost: 0.0000001 });
          expect(result).toBeNull();
      });
  });
});

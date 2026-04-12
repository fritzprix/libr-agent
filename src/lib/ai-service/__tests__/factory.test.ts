import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { AIServiceFactory } from '../factory';
import { AIServiceProvider } from '../types';
import { GroqService } from '../groq';
import { OpenAIService } from '../openai';
import { AnthropicService } from '../anthropic';
import { GeminiService } from '../gemini';
import { FireworksService } from '../fireworks';
import { CerebrasService } from '../cerebras';
import { OllamaService } from '../ollama';
import { OpenRouterService } from '../openrouter';
import { EmptyAIService } from '../empty';

// Mock the config manager to ensure reliable API key requirements
vi.mock('../../llm-config-manager', () => {
  return {
    LLMConfigManager: vi.fn().mockImplementation(() => {
      return {
        getProviders: vi.fn().mockReturnValue({
          groq: { requiresApiKey: true },
          openai: { requiresApiKey: true },
          anthropic: { requiresApiKey: true },
          gemini: { requiresApiKey: true },
          fireworks: { requiresApiKey: true },
          cerebras: { requiresApiKey: true },
          ollama: { requiresApiKey: false },
          openrouter: { requiresApiKey: true },
        })
      };
    })
  };
});

// Mock the services to avoid making real API calls or complex initializations
vi.mock('../groq');
vi.mock('../openai');
vi.mock('../anthropic');
vi.mock('../gemini');
vi.mock('../fireworks');
vi.mock('../cerebras');
vi.mock('../ollama');
vi.mock('../openrouter');

// Define mock classes
vi.mocked(GroqService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as GroqService));
vi.mocked(OpenAIService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as OpenAIService));
vi.mocked(AnthropicService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as AnthropicService));
vi.mocked(GeminiService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as GeminiService));
vi.mocked(FireworksService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as FireworksService));
vi.mocked(CerebrasService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as CerebrasService));
vi.mocked(OllamaService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as OllamaService));
vi.mocked(OpenRouterService).mockImplementation(() => ({ dispose: vi.fn() } as unknown as OpenRouterService));

describe('AIServiceFactory', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // Reset the internal instances map before each test
    // @ts-expect-error accessing private property for testing
    AIServiceFactory.instances.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('Service Creation', () => {
    it('should create a GroqService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Groq, 'test-key');
      expect(GroqService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create an OpenAIService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');
      expect(OpenAIService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create an AnthropicService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Anthropic, 'test-key');
      expect(AnthropicService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create a GeminiService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Gemini, 'test-key');
      expect(GeminiService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create a FireworksService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Fireworks, 'test-key');
      expect(FireworksService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create a CerebrasService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Cerebras, 'test-key');
      expect(CerebrasService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create an OllamaService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.Ollama, 'test-key');
      expect(OllamaService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should create an OpenRouterService when requested', () => {
      AIServiceFactory.getService(AIServiceProvider.OpenRouter, 'test-key');
      expect(OpenRouterService).toHaveBeenCalledWith('test-key', undefined);
    });

    it('should return EmptyAIService for unknown provider', () => {
      const service = AIServiceFactory.getService('unknown-provider' as AIServiceProvider, 'test-key');
      expect(service).toBeInstanceOf(EmptyAIService);
    });

    it('should return EmptyAIService if service constructor throws an error', () => {
      // Force GroqService constructor to throw an error
      vi.mocked(GroqService).mockImplementationOnce(() => {
        throw new Error('Constructor failed');
      });
      const service = AIServiceFactory.getService(AIServiceProvider.Groq, 'test-key');
      expect(service).toBeInstanceOf(EmptyAIService);
    });
  });

  describe('Caching and Lifecycle', () => {
    it('should return the cached instance if called multiple times with the same parameters', () => {
      const service1 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');
      const service2 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');

      expect(service1).toBe(service2);
      expect(OpenAIService).toHaveBeenCalledTimes(1);
    });

    it('should create a new instance if API key is different', () => {
      const service1 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key-1');
      const service2 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key-2');

      expect(service1).not.toBe(service2);
      expect(OpenAIService).toHaveBeenCalledTimes(2);
    });

    it('should create a new instance if baseUrl is different', () => {
      const service1 = AIServiceFactory.getService(
        AIServiceProvider.Ollama,
        '',
        { baseUrl: 'http://old-host:11434' },
      );
      const service2 = AIServiceFactory.getService(
        AIServiceProvider.Ollama,
        '',
        { baseUrl: 'http://new-host:11434' },
      );

      expect(service1).not.toBe(service2);
      expect(OllamaService).toHaveBeenCalledTimes(2);
      expect(OllamaService).toHaveBeenNthCalledWith(1, 'ollama-local', {
        baseUrl: 'http://old-host:11434',
      });
      expect(OllamaService).toHaveBeenNthCalledWith(2, 'ollama-local', {
        baseUrl: 'http://new-host:11434',
      });
    });

    it('should expire cached instance after INSTANCE_TTL', () => {
      const service1 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');

      // Advance time by 1 hour and 1 ms
      vi.advanceTimersByTime(1000 * 60 * 60 + 1);

      const service2 = AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');

      expect(service1).not.toBe(service2);
      expect(OpenAIService).toHaveBeenCalledTimes(2);
    });

    it('should dispose expired instances during cleanup', () => {
      const disposeSpy = vi.fn();
      vi.mocked(OpenAIService).mockImplementationOnce(() => {
        return { dispose: disposeSpy } as unknown as OpenAIService;
      });

      AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key');

      // Advance time past TTL
      vi.advanceTimersByTime(1000 * 60 * 60 + 1);

      // Trigger cleanup implicitly via another getService call
      AIServiceFactory.getService(AIServiceProvider.Groq, 'other-key');

      expect(disposeSpy).toHaveBeenCalledTimes(1);
    });

    it('should explicitly dispose all instances when disposeAll is called', () => {
      const disposeSpy1 = vi.fn();
      const disposeSpy2 = vi.fn();

      vi.mocked(OpenAIService).mockImplementationOnce(() => {
        return { dispose: disposeSpy1 } as unknown as OpenAIService;
      });
      vi.mocked(GroqService).mockImplementationOnce(() => {
        return { dispose: disposeSpy2 } as unknown as GroqService;
      });

      AIServiceFactory.getService(AIServiceProvider.OpenAI, 'test-key-1');
      AIServiceFactory.getService(AIServiceProvider.Groq, 'test-key-2');

      AIServiceFactory.disposeAll();

      expect(disposeSpy1).toHaveBeenCalledTimes(1);
      expect(disposeSpy2).toHaveBeenCalledTimes(1);

      // @ts-expect-error accessing private property for testing
      expect(AIServiceFactory.instances.size).toBe(0);
    });
  });

  describe('API Key overrides', () => {
    it('should use a dummy key if provider does not require an API key and none is provided', () => {
      // In LLMConfigManager, Ollama does not require an API key.
      AIServiceFactory.getService(AIServiceProvider.Ollama, '');
      expect(OllamaService).toHaveBeenCalledWith('ollama-local', undefined);
    });
  });
});

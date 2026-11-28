import { describe, expect, it, vi } from 'vitest';
import { estimateContextWindow, supportsTools } from '../model-capabilities';
import { AIServiceProvider } from '../types';
import { OpenAIService } from '../openai';

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

interface CapabilityCase {
  provider: AIServiceProvider | string;
  modelName: string;
  tools: boolean;
  contextWindow: number;
}

// Table of known models pinned to the exact capability output of the
// previous factory-delegated implementation (regression guard for the
// static lookup refactor, #3092).
const CAPABILITY_TABLE: CapabilityCase[] = [
  // OpenAI
  { provider: AIServiceProvider.OpenAI, modelName: 'gpt-4o', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'gpt-4o-2024-08-06', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'gpt-4.1', tools: true, contextWindow: 1000000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'gpt-4.1-mini', tools: true, contextWindow: 1000000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'gpt-3.5-turbo', tools: true, contextWindow: 8192 },
  { provider: AIServiceProvider.OpenAI, modelName: 'o1', tools: true, contextWindow: 8192 },
  { provider: AIServiceProvider.OpenAI, modelName: 'o3-mini', tools: true, contextWindow: 200000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'o4-mini', tools: true, contextWindow: 200000 },
  { provider: AIServiceProvider.OpenAI, modelName: 'orca-mini', tools: false, contextWindow: 8192 },
  // Anthropic
  { provider: AIServiceProvider.Anthropic, modelName: 'claude-3-5-sonnet-20240620', tools: true, contextWindow: 200000 },
  { provider: AIServiceProvider.Anthropic, modelName: 'claude-opus-4-1', tools: true, contextWindow: 100000 },
  { provider: AIServiceProvider.Anthropic, modelName: 'claude-2', tools: false, contextWindow: 100000 },
  // Gemini
  { provider: AIServiceProvider.Gemini, modelName: 'gemini-1.5-pro', tools: true, contextWindow: 2000000 },
  { provider: AIServiceProvider.Gemini, modelName: 'gemini-1.5-flash', tools: true, contextWindow: 1000000 },
  { provider: AIServiceProvider.Gemini, modelName: 'gemini-2.0-flash', tools: true, contextWindow: 1000000 },
  { provider: AIServiceProvider.Gemini, modelName: 'gemini-2.5-pro', tools: true, contextWindow: 1000000 },
  { provider: AIServiceProvider.Gemini, modelName: 'gemini-1.0-pro', tools: false, contextWindow: 1000000 },
  // Groq
  { provider: AIServiceProvider.Groq, modelName: 'llama3-8b-8192', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Groq, modelName: 'llama-3.1-70b-versatile', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.Groq, modelName: 'llama-3.1-8b-instant', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.Groq, modelName: 'mixtral-8x7b-32768', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Groq, modelName: 'gemma2-9b-it', tools: false, contextWindow: 32768 },
  // Ollama
  { provider: AIServiceProvider.Ollama, modelName: 'llama3.1:8b', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Ollama, modelName: 'llama-3.1:8b', tools: false, contextWindow: 128000 },
  { provider: AIServiceProvider.Ollama, modelName: 'llama3.2:3b', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Ollama, modelName: 'qwen2.5:7b', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Ollama, modelName: 'deepseek-r1:8b', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Ollama, modelName: 'phi3:latest', tools: false, contextWindow: 32768 },
  // Cerebras
  { provider: AIServiceProvider.Cerebras, modelName: 'llama3.1-8b', tools: true, contextWindow: 131072 },
  { provider: AIServiceProvider.Cerebras, modelName: 'llama3.1-70b', tools: true, contextWindow: 131072 },
  { provider: AIServiceProvider.Cerebras, modelName: 'llama3.1-400b', tools: true, contextWindow: 8192 },
  { provider: AIServiceProvider.Cerebras, modelName: 'gpt-oss-120b', tools: false, contextWindow: 8192 },
  // Fireworks
  { provider: AIServiceProvider.Fireworks, modelName: 'accounts/fireworks/models/llama-v3p1-405b', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.Fireworks, modelName: 'qwen2.5-32b-instruct', tools: true, contextWindow: 32768 },
  { provider: AIServiceProvider.Fireworks, modelName: 'gemma2-9b-it', tools: true, contextWindow: 128000 },
  // OpenRouter
  { provider: AIServiceProvider.OpenRouter, modelName: 'meta-llama/llama-3.1-70b-instruct', tools: true, contextWindow: 128000 },
  { provider: AIServiceProvider.OpenRouter, modelName: 'anything', tools: true, contextWindow: 128000 },
  // Custom OpenAI-compatible endpoints use the OpenAI ruleset.
  { provider: 'custom:local-llm', modelName: 'gpt-4o-mini', tools: true, contextWindow: 128000 },
  { provider: 'custom:local-llm', modelName: 'Meta-Llama-3.1-8B-Instruct', tools: false, contextWindow: 8192 },
  // Empty / unknown providers have no capabilities.
  { provider: AIServiceProvider.Empty, modelName: 'gpt-4o', tools: false, contextWindow: 0 },
  { provider: 'totally-unknown-provider', modelName: 'gpt-4o', tools: false, contextWindow: 0 },
];

describe('model-capabilities', () => {
  it.each(CAPABILITY_TABLE)(
    'supportsTools/estimateContextWindow for $provider + $modelName',
    ({ provider, modelName, tools, contextWindow }) => {
      expect(supportsTools(modelName, provider)).toBe(tools);
      expect(estimateContextWindow(modelName, provider)).toBe(contextWindow);
    },
  );

  it('matches only known OpenAI o-series prefixes for tool support', () => {
    expect(OpenAIService.supportsToolsForModel('o3-mini')).toBe(true);
    expect(OpenAIService.supportsToolsForModel('o4-mini')).toBe(true);
    expect(OpenAIService.supportsToolsForModel('orca-mini')).toBe(false);
    expect(OpenAIService.supportsToolsForModel('gpt-4o')).toBe(true);
  });
});

import { describe, expect, it } from 'vitest';
import {
  estimateContextWindow,
  supportsTools,
} from '../model-capabilities';
import { AIServiceProvider } from '../types';

describe('model-capabilities', () => {
  // Table test pinning the results the AIServiceFactory capability
  // delegation produced before the per-provider logic moved to
  // `provider-capabilities` (#3092).
  const TOOL_SUPPORT_CASES: Array<[string, AIServiceProvider | string, boolean]> =
    [
      ['gpt-4o', AIServiceProvider.OpenAI, true],
      ['gpt-4.1', AIServiceProvider.OpenAI, true],
      ['gpt-3.5-turbo', AIServiceProvider.OpenAI, true],
      ['o3-mini', AIServiceProvider.OpenAI, true],
      ['o4-mini', AIServiceProvider.OpenAI, true],
      ['orca-mini', AIServiceProvider.OpenAI, false],
      ['claude-3-5-sonnet', AIServiceProvider.Anthropic, true],
      ['claude-opus-4-20250514', AIServiceProvider.Anthropic, true],
      ['claude-2.1', AIServiceProvider.Anthropic, false],
      ['gemini-2.5-pro', AIServiceProvider.Gemini, true],
      ['gemini-1.5-flash', AIServiceProvider.Gemini, true],
      ['gemini-pro', AIServiceProvider.Gemini, false],
      ['llama-3.1-70b-versatile', AIServiceProvider.Groq, true],
      ['mixtral-8x7b-32768', AIServiceProvider.Groq, true],
      ['gemma2-9b-it', AIServiceProvider.Groq, false],
      ['accounts/fireworks/models/llama-v3p1', AIServiceProvider.Fireworks, true],
      ['llama3.1-8b', AIServiceProvider.Cerebras, true],
      ['llama3-8b', AIServiceProvider.Cerebras, false],
      ['llama3.1:8b', AIServiceProvider.Ollama, true],
      ['qwen2.5:7b', AIServiceProvider.Ollama, true],
      ['orca-mini', AIServiceProvider.Ollama, false],
      ['anthropic/claude-3.5-sonnet', AIServiceProvider.OpenRouter, true],
      ['anything', AIServiceProvider.Empty, false],
      ['gpt-4o', 'custom:local-llm', true],
      ['llama3.1', 'custom:local-llm', false],
      ['gpt-4o', 'not-a-provider', false],
    ];

  const CONTEXT_WINDOW_CASES: Array<
    [string, AIServiceProvider | string, number]
  > = [
    ['gpt-4.1', AIServiceProvider.OpenAI, 1000000],
    ['gpt-4o', AIServiceProvider.OpenAI, 128000],
    ['o3', AIServiceProvider.OpenAI, 200000],
    ['o4-mini', AIServiceProvider.OpenAI, 200000],
    ['gpt-3.5-turbo', AIServiceProvider.OpenAI, 8192],
    ['claude-3-5-sonnet', AIServiceProvider.Anthropic, 200000],
    ['claude-opus-4-20250514', AIServiceProvider.Anthropic, 100000],
    ['gemini-1.5-pro', AIServiceProvider.Gemini, 2000000],
    ['gemini-1.5-flash', AIServiceProvider.Gemini, 1000000],
    ['gemini-2.5-pro', AIServiceProvider.Gemini, 1000000],
    ['llama-3.1-70b-versatile', AIServiceProvider.Groq, 128000],
    ['llama3-70b-8192', AIServiceProvider.Groq, 32768],
    ['accounts/fireworks/models/qwen2p5-72b', AIServiceProvider.Fireworks, 32768],
    ['accounts/fireworks/models/llama-v3p1-70b', AIServiceProvider.Fireworks, 128000],
    ['llama3.1-70b', AIServiceProvider.Cerebras, 131072],
    ['llama3-8b', AIServiceProvider.Cerebras, 8192],
    ['llama-3.1:8b', AIServiceProvider.Ollama, 128000],
    ['mistral-nemo:latest', AIServiceProvider.Ollama, 128000],
    ['qwen2.5:7b', AIServiceProvider.Ollama, 32768],
    ['any-model', AIServiceProvider.OpenRouter, 128000],
    ['any-model', AIServiceProvider.Empty, 0],
    ['gpt-4o', 'custom:local-llm', 128000],
    ['gpt-4.1', 'custom:local-llm', 1000000],
    ['any-model', 'not-a-provider', 0],
  ];

  it.each(TOOL_SUPPORT_CASES)(
    'supportsTools(%j, %j) === %j',
    (model, provider, expected) => {
      expect(supportsTools(model, provider)).toBe(expected);
    },
  );

  it.each(CONTEXT_WINDOW_CASES)(
    'estimateContextWindow(%j, %j) === %j',
    (model, provider, expected) => {
      expect(estimateContextWindow(model, provider)).toBe(expected);
    },
  );
});

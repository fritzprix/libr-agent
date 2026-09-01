import { describe, expect, it } from 'vitest';
import { mapThinkingBudget } from '../thinking-effort-mapping';
import { AIServiceProvider } from '../types';

describe('mapThinkingBudget', () => {
  it('returns disabled for undefined and zero budgets', () => {
    expect(mapThinkingBudget(AIServiceProvider.OpenAI, undefined)).toEqual({
      enabled: false,
    });
    expect(mapThinkingBudget(AIServiceProvider.Gemini, 0)).toEqual({
      enabled: false,
    });
  });

  it('maps dynamic budget (-1) for thinking-capable providers', () => {
    expect(mapThinkingBudget(AIServiceProvider.OpenAI, -1)).toEqual({
      enabled: true,
      reasoningEffort: 'medium',
    });
    expect(mapThinkingBudget(AIServiceProvider.Anthropic, -1)).toEqual({
      enabled: true,
      extendedThinking: true,
    });
    expect(mapThinkingBudget(AIServiceProvider.Gemini, -1)).toEqual({
      enabled: true,
      thinkingBudget: -1,
    });
    expect(mapThinkingBudget(AIServiceProvider.Ollama, -1)).toEqual({
      enabled: true,
      reasoningEffort: 'medium',
    });
  });

  it('derives effort levels from explicit token budgets', () => {
    expect(mapThinkingBudget(AIServiceProvider.OpenAI, 1024)).toEqual({
      enabled: true,
      reasoningEffort: 'low',
    });
    expect(mapThinkingBudget(AIServiceProvider.OpenAI, 8192)).toEqual({
      enabled: true,
      reasoningEffort: 'medium',
    });
    expect(mapThinkingBudget(AIServiceProvider.OpenAI, 24576)).toEqual({
      enabled: true,
      reasoningEffort: 'high',
    });
  });

  it('passes explicit Gemini budgets through unchanged', () => {
    expect(mapThinkingBudget(AIServiceProvider.Gemini, 4096)).toEqual({
      enabled: true,
      thinkingBudget: 4096,
    });
  });

  it('enables Anthropic extended thinking for any positive budget', () => {
    expect(mapThinkingBudget(AIServiceProvider.Anthropic, 1024)).toEqual({
      enabled: true,
      extendedThinking: true,
    });
  });

  it('returns disabled for providers without thinking support', () => {
    expect(mapThinkingBudget(AIServiceProvider.Groq, 8192)).toEqual({
      enabled: false,
    });
    expect(mapThinkingBudget(AIServiceProvider.Empty, -1)).toEqual({
      enabled: false,
    });
  });

  it('uses effort boundaries at 2048 and 16384 tokens', () => {
    expect(
      mapThinkingBudget(AIServiceProvider.Ollama, 2048).reasoningEffort,
    ).toBe('low');
    expect(
      mapThinkingBudget(AIServiceProvider.Ollama, 2049).reasoningEffort,
    ).toBe('medium');
    expect(
      mapThinkingBudget(AIServiceProvider.Ollama, 16384).reasoningEffort,
    ).toBe('medium');
    expect(
      mapThinkingBudget(AIServiceProvider.Ollama, 16385).reasoningEffort,
    ).toBe('high');
  });
});

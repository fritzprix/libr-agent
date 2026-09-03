import { describe, expect, it } from 'vitest';
import {
  mapThinkingEffort,
  normalizeThinkingEffort,
} from '../thinking-effort-mapping';
import { AIServiceProvider } from '../types';

describe('normalizeThinkingEffort', () => {
  it('passes through valid effort values', () => {
    expect(normalizeThinkingEffort('medium')).toBe('medium');
    expect(normalizeThinkingEffort('auto')).toBe('auto');
  });

  it('migrates legacy numeric budgets', () => {
    expect(normalizeThinkingEffort(undefined, 0)).toBe('off');
    expect(normalizeThinkingEffort(undefined, -1)).toBe('auto');
    expect(normalizeThinkingEffort(undefined, 1024)).toBe('low');
    expect(normalizeThinkingEffort(undefined, 8192)).toBe('medium');
    expect(normalizeThinkingEffort(undefined, 24576)).toBe('high');
  });

  it('defaults unknown values to off', () => {
    expect(normalizeThinkingEffort('invalid')).toBe('off');
  });
});

describe('mapThinkingEffort', () => {
  it('returns disabled for off and undefined', () => {
    expect(mapThinkingEffort(AIServiceProvider.OpenAI, 'off')).toEqual({
      enabled: false,
    });
    expect(mapThinkingEffort(AIServiceProvider.Gemini, undefined)).toEqual({
      enabled: false,
    });
  });

  it('maps effort presets for OpenAI-compatible providers', () => {
    expect(mapThinkingEffort(AIServiceProvider.OpenAI, 'low')).toEqual({
      enabled: true,
      reasoningEffort: 'low',
    });
    expect(mapThinkingEffort(AIServiceProvider.Ollama, 'high')).toEqual({
      enabled: true,
      reasoningEffort: 'high',
    });
    expect(mapThinkingEffort(AIServiceProvider.OpenAI, 'auto')).toEqual({
      enabled: true,
      reasoningEffort: 'medium',
    });
  });

  it('maps Anthropic effort to extended thinking', () => {
    expect(mapThinkingEffort(AIServiceProvider.Anthropic, 'medium')).toEqual({
      enabled: true,
      extendedThinking: true,
    });
  });

  it('maps Gemini effort to internal token budgets', () => {
    expect(mapThinkingEffort(AIServiceProvider.Gemini, 'medium')).toEqual({
      enabled: true,
      thinkingBudget: 8192,
    });
    expect(mapThinkingEffort(AIServiceProvider.Gemini, 'auto')).toEqual({
      enabled: true,
      thinkingBudget: -1,
    });
  });

  it('enables Groq thinking via reasoning_format (no effort level)', () => {
    expect(mapThinkingEffort(AIServiceProvider.Groq, 'medium')).toEqual({
      enabled: true,
    });
  });

  it('returns disabled for providers without thinking mapping', () => {
    expect(mapThinkingEffort(AIServiceProvider.Empty, 'medium')).toEqual({
      enabled: false,
    });
  });
});

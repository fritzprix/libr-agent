import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  getStoredModelCache,
  setStoredModelCache,
  clearStoredModelCache,
} from '../model-cache-storage';
import type { ModelInfo } from '@/lib/llm-config-manager';

describe('model-cache-storage', () => {
  beforeEach(() => {
    clearStoredModelCache();
  });

  afterEach(() => {
    clearStoredModelCache();
  });

  it('saves and retrieves model cache per provider from localStorage', () => {
    const dummyModels: Record<string, ModelInfo> = {
      'gpt-4o': {
        id: 'gpt-4o',
        name: 'GPT-4o',
        contextWindow: 128000,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: { input: 2.5, output: 10 },
        description: 'OpenAI flagship model',
      },
    };

    setStoredModelCache('openai', dummyModels);
    const retrieved = getStoredModelCache('openai');

    expect(retrieved).not.toBeNull();
    expect(retrieved?.['gpt-4o']?.name).toBe('GPT-4o');
  });

  it('returns null when no cache is stored for provider', () => {
    const retrieved = getStoredModelCache('nonexistent_provider');
    expect(retrieved).toBeNull();
  });

  it('clears stored model cache for specific provider and all providers', () => {
    const dummy: Record<string, ModelInfo> = {
      model1: {
        id: 'm1',
        name: 'M1',
        contextWindow: 4096,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: { input: 0, output: 0 },
        description: '',
      },
    };

    setStoredModelCache('openai', dummy);
    setStoredModelCache('anthropic', dummy);

    clearStoredModelCache('openai');
    expect(getStoredModelCache('openai')).toBeNull();
    expect(getStoredModelCache('anthropic')).not.toBeNull();

    clearStoredModelCache();
    expect(getStoredModelCache('anthropic')).toBeNull();
  });
});

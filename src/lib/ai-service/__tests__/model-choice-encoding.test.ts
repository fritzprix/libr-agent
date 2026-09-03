import { describe, expect, it } from 'vitest';

import {
  decodeModelChoice,
  encodeModelChoice,
} from '@/lib/ai-service/model-choice-encoding';

describe('model-choice-encoding', () => {
  it('round-trips provider and model', () => {
    const encoded = encodeModelChoice('anthropic', 'claude-sonnet-4');
    expect(encoded).toBe('anthropic:::claude-sonnet-4');
    expect(decodeModelChoice(encoded)).toEqual({
      provider: 'anthropic',
      model: 'claude-sonnet-4',
    });
  });

  it('supports custom provider ids and slash model ids', () => {
    const encoded = encodeModelChoice('custom:local1', 'meta/llama-3.1-70b');
    expect(decodeModelChoice(encoded)).toEqual({
      provider: 'custom:local1',
      model: 'meta/llama-3.1-70b',
    });
  });

  it('returns null for malformed values', () => {
    expect(decodeModelChoice('')).toBeNull();
    expect(decodeModelChoice('openai')).toBeNull();
    expect(decodeModelChoice('openai::gpt-4o')).toBeNull();
  });
});

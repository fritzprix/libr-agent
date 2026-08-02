import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  clearLastSelectedModel,
  getLastSelectedModel,
  setLastSelectedModel,
} from '../last-selected-model-storage';

describe('last-selected-model-storage', () => {
  beforeEach(() => {
    clearLastSelectedModel();
  });

  afterEach(() => {
    clearLastSelectedModel();
  });

  it('saves and retrieves the last selected model per provider', () => {
    setLastSelectedModel('custom:nim', 'meta/llama-3.1-70b-instruct');
    setLastSelectedModel('openai', 'gpt-4o');

    expect(getLastSelectedModel('custom:nim')).toBe(
      'meta/llama-3.1-70b-instruct',
    );
    expect(getLastSelectedModel('openai')).toBe('gpt-4o');
  });

  it('ignores empty model values', () => {
    setLastSelectedModel('openai', '   ');
    expect(getLastSelectedModel('openai')).toBeNull();
  });

  it('clears one provider without affecting others', () => {
    setLastSelectedModel('openai', 'gpt-4o');
    setLastSelectedModel('anthropic', 'claude-sonnet-4');

    clearLastSelectedModel('openai');
    expect(getLastSelectedModel('openai')).toBeNull();
    expect(getLastSelectedModel('anthropic')).toBe('claude-sonnet-4');
  });
});

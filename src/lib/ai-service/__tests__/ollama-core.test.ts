import { describe, it, expect } from 'vitest';
import {
  createThinkTagStreamState,
  processChunk,
  noopLogger,
} from '../ollama-core';

describe('ollama-core processChunk', () => {
  it('should not trim whitespace from thinking content in chunks', () => {
    // Case 1: Thinking in 'thinking' field (native)
    const chunkWithThinkingIds = {
      message: {
        thinking: '  thinking with spaces  ',
      },
    };

    const result = processChunk(chunkWithThinkingIds, noopLogger);

    expect(result?.thinking).toBe('  thinking with spaces  ');
  });

  it('should not trim whitespace from content mixed with thinking tags', () => {
    // Case 2: Thinking in 'content' field with tags
    const chunkMixed = {
      message: {
        content: '<think>  thinking inside tags  </think>',
      },
    };

    const result = processChunk(chunkMixed, noopLogger);

    expect(result?.thinking).toBe('  thinking inside tags  ');
  });

  it('should not trim whitespace from content after thinking tags', () => {
    const chunkMixed = {
      message: {
        content: '<think>thought</think>  content with spaces  ',
      },
    };

    const result = processChunk(chunkMixed, noopLogger);

    expect(result?.content).toBe('  content with spaces  ');
  });

  it('should classify unclosed think tags as thinking, not content', () => {
    const result = processChunk(
      {
        message: {
          content: '<think>\nincomplete reasoning without close tag',
        },
      },
      noopLogger,
    );

    expect(result?.content).toBeUndefined();
    expect(result?.thinking).toBe('\nincomplete reasoning without close tag');
  });

  it('should keep think tag state across streamed content deltas', () => {
    const thinkTagState = createThinkTagStreamState();

    const first = processChunk(
      { message: { content: '<think>alpha' } },
      noopLogger,
      undefined,
      thinkTagState,
    );
    const second = processChunk(
      { message: { content: ' beta</think>gamma' } },
      noopLogger,
      undefined,
      thinkTagState,
    );

    expect(first?.thinking).toBe('alpha');
    expect(first?.content).toBeUndefined();
    expect(second?.thinking).toBe(' beta');
    expect(second?.content).toBe('gamma');
  });

  it('should extract usage metrics from final chunk', () => {
    const chunk = {
      done: true,
      prompt_eval_count: 127,
      eval_count: 543,
      eval_duration: 12847362000,
      load_duration: 150000000,
    };

    const result = processChunk(chunk, noopLogger);

    expect(result?.usage).toBeDefined();
    expect(result?.usage?.promptTokens).toBe(127);
    expect(result?.usage?.completionTokens).toBe(543);
    expect(result?.usage?.totalTokens).toBe(670);
    expect(result?.usage?.details?.evalDuration).toBeCloseTo(12847.36, 1);
  });

  it('should not include usage in non-final chunks', () => {
    const chunk = {
      done: false,
      message: { content: 'Hello', role: 'assistant' },
    };

    const result = processChunk(chunk, noopLogger);

    expect(result?.usage).toBeUndefined();
  });
});

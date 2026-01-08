import { describe, it, expect } from 'vitest';
import { processChunk, noopLogger } from '../ollama-core';

describe('ollama-core processChunk', () => {
    it('should not trim whitespace from thinking content in chunks', () => {
        // Case 1: Thinking in 'thinking' field (native)
        const chunkWithThinkingIds = {
            message: {
                thinking: '  thinking with spaces  ',
            }
        };

        const result = processChunk(chunkWithThinkingIds, noopLogger);

        expect(result?.thinking).toBe('  thinking with spaces  ');
    });

    it('should not trim whitespace from content mixed with thinking tags', () => {
        // Case 2: Thinking in 'content' field with tags
        const chunkMixed = {
            message: {
                content: '<think>  thinking inside tags  </think>',
            }
        };

        const result = processChunk(chunkMixed, noopLogger);

        expect(result?.thinking).toBe('  thinking inside tags  ');
    });

    it('should not trim whitespace from content after thinking tags', () => {
        const chunkMixed = {
            message: {
                content: '<think>thought</think>  content with spaces  ',
            }
        };

        const result = processChunk(chunkMixed, noopLogger);

        expect(result?.content).toBe('  content with spaces  ');
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

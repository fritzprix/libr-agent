
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


        const resultJson = processChunk(chunkWithThinkingIds, noopLogger);
        const result = JSON.parse(resultJson!);

        expect(result.thinking).toBe('  thinking with spaces  ');
    });

    it('should not trim whitespace from content mixed with thinking tags', () => {
        // Case 2: Thinking in 'content' field with tags
        const chunkMixed = {
            message: {
                content: '<think>  thinking inside tags  </think>',
            }
        };


        const resultJson = processChunk(chunkMixed, noopLogger);
        const result = JSON.parse(resultJson!);

        expect(result.thinking).toBe('  thinking inside tags  ');
    });

    it('should not trim whitespace from content after thinking tags', () => {
        const chunkMixed = {
            message: {
                content: '<think>thought</think>  content with spaces  ',
            }
        };


        const resultJson = processChunk(chunkMixed, noopLogger);
        const result = JSON.parse(resultJson!);

        expect(result.content).toBe('  content with spaces  ');
    });
});

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { selectMessagesWithinContext, estimateTextTokens } from '../token-utils';
import { llmConfigManager } from '../llm-config-manager';
import type { Message } from '@/models/chat';
import type { ModelInfo } from '../llm-config-manager';

// Mock dependencies
vi.mock('@dqbd/tiktoken', () => ({
  get_encoding: () => ({
    encode: (text: string) => new Uint8Array(text.length), // Simple 1 char = 1 token mock
    free: vi.fn(),
  }),
}));

vi.mock('../llm-config-manager', () => ({
  llmConfigManager: {
    getModel: vi.fn(),
  },
}));

vi.mock('../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('token-utils', () => {
  describe('estimateTextTokens', () => {
    it('should return length of text as token count (mocked)', () => {
      expect(estimateTextTokens('hello')).toBe(5);
    });
  });

  describe('selectMessagesWithinContext', () => {
    const createMessage = (id: string, content: string, role: 'user' | 'assistant' = 'user'): Message => ({
      id,
      role,
      content: [{ type: 'text', text: content }],
      sessionId: 'session-1',
      threadId: 'thread-1',
    });

    beforeEach(() => {
      vi.clearAllMocks();
      // Default mock for getModel
      vi.mocked(llmConfigManager.getModel).mockReturnValue({
      contextWindow: 100,
    } as ModelInfo);
  });

  it('should return all messages if they fit within the limit', () => {
      const messages = [
        createMessage('1', 'short'),
        createMessage('2', 'message'),
      ];

      // 'user: short' = 11 tokens
      // 'user: message' = 13 tokens
      // Total ~24 tokens. Context 100 * 0.9 = 90. Should fit.

      const result = selectMessagesWithinContext(messages, 'openai', 'gpt-4');
      expect(result).toHaveLength(2);
      expect(result[0].id).toBe('1');
      expect(result[1].id).toBe('2');
    });

    it('should truncate older messages if limit is exceeded', () => {
      // Set context window such that limit > 1024
      // 2000 * 0.9 = 1800.
      vi.mocked(llmConfigManager.getModel).mockReturnValue({
        contextWindow: 2000,
      } as ModelInfo);
      
      const longMessageContent = 'a'.repeat(1500); // 1500 tokens
      const mediumMessageContent = 'b'.repeat(400); // 400 tokens
      // Total 1900 > 1800.

      const messages = [
        createMessage('1', longMessageContent),
        createMessage('2', mediumMessageContent),
      ];

      const result = selectMessagesWithinContext(messages, 'openai', 'gpt-4');
      
      // Should keep msg 2 (400) and drop msg 1 (1500) because 400+1500 > 1800
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('2');
    });

    it('should reserve tokens for system prompt and tools', () => {
      // Context 2000 -> 1800 limit.
      vi.mocked(llmConfigManager.getModel).mockReturnValue({
        contextWindow: 2000,
      } as ModelInfo);
      
      const messages = [createMessage('1', 'hello')]; // "user: hello" = 11 tokens

      // System prompt 500 tokens
      // Tools 500 tokens
      // Available = 1800 - 1000 = 800.
      // Message fits.

      const result = selectMessagesWithinContext(messages, 'openai', 'gpt-4', undefined, {
        systemPrompt: 'a'.repeat(500),
        toolsJson: 'b'.repeat(500),
      });

      expect(result).toHaveLength(1);
    });

    it('should handle missing model info gracefully', () => {
      vi.mocked(llmConfigManager.getModel).mockReturnValue(null);
      const messages = [createMessage('1', 'hello')];

      const result = selectMessagesWithinContext(messages, 'unknown', 'model');
      expect(result).toEqual(messages);
    });

    it('should respect maxTokens parameter if provided', () => {
       // Force maxTokens to be > 1024 to bypass the minimum limit check
       const maxTokens = 1100;
       
       const longMessageContent = 'a'.repeat(1000); // 1000 tokens
       const mediumMessageContent = 'b'.repeat(200); // 200 tokens
       // Total 1200 > 1100.

       const messages = [
        createMessage('1', longMessageContent), 
        createMessage('2', mediumMessageContent),
      ];
      
      const result = selectMessagesWithinContext(messages, 'openai', 'gpt-4', maxTokens);
      
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('2');
    });
  });
});

import { describe, it, expect, vi } from 'vitest';
import {
  sanitizeJsonField,
  sanitizeMessage,
  allToolUsePairsAreValid,
  removeInvalidToolUseAndToolResponse,
} from '../use-ai-service';
import type { Message } from '@/models/chat';

// Mock logger
vi.mock('../lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('use-ai-service helpers', () => {
  describe('sanitizeJsonField', () => {
    it('should return valid JSON string as-is', () => {
      const validJson = '{"key": "value"}';
      expect(sanitizeJsonField(validJson)).toBe(validJson);
    });

    it('should stringify invalid JSON string', () => {
      const invalidJson = 'invalid json';
      // JSON.stringify("invalid json") -> "\"invalid json\""
      expect(sanitizeJsonField(invalidJson)).toBe(JSON.stringify(invalidJson));
    });
  });

  describe('sanitizeMessage', () => {
    it('should sanitize tool call arguments', () => {
      const message: Message = {
        id: '1',
        role: 'assistant',
        content: [],
        sessionId: 's1',
        threadId: 't1',
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: {
              name: 'test',
              arguments: 'invalid json',
            },
          },
        ],
      };

      const sanitized = sanitizeMessage(message);
      expect(sanitized.tool_calls?.[0].function.arguments).toBe(JSON.stringify('invalid json'));
    });

    it('should sanitize thinking field', () => {
      const message: Message = {
        id: '1',
        role: 'assistant',
        content: [],
        sessionId: 's1',
        threadId: 't1',
        thinking: 'invalid json',
      };

      const sanitized = sanitizeMessage(message);
      expect(sanitized.thinking).toBe(JSON.stringify('invalid json'));
    });
  });

  describe('allToolUsePairsAreValid', () => {
    it('should return true for valid pairs', () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'assistant',
          content: [],
          sessionId: 's1',
          threadId: 't1',
          tool_calls: [{ id: 'c1', type: 'function', function: { name: 'f', arguments: '{}' } }],
        },
        {
          id: '2',
          role: 'tool',
          content: [{ type: 'text', text: 'result' }],
          sessionId: 's1',
          threadId: 't1',
          tool_call_id: 'c1',
        },
      ];
      expect(allToolUsePairsAreValid(messages)).toBe(true);
    });

    it('should return false for dangling tool call', () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'assistant',
          content: [],
          sessionId: 's1',
          threadId: 't1',
          tool_calls: [{ id: 'c1', type: 'function', function: { name: 'f', arguments: '{}' } }],
        },
        // Missing tool response
      ];
      expect(allToolUsePairsAreValid(messages)).toBe(false);
    });
  });

  describe('removeInvalidToolUseAndToolResponse', () => {
    it('should remove dangling tool calls', () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'assistant',
          content: [],
          sessionId: 's1',
          threadId: 't1',
          tool_calls: [{ id: 'c1', type: 'function', function: { name: 'f', arguments: '{}' } }],
        },
        {
          id: '3',
          role: 'user',
          content: [{ type: 'text', text: 'next' }],
          sessionId: 's1',
          threadId: 't1',
        },
      ];

      const result = removeInvalidToolUseAndToolResponse(messages);
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('3');
    });

    it('should keep valid pairs', () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'assistant',
          content: [],
          sessionId: 's1',
          threadId: 't1',
          tool_calls: [{ id: 'c1', type: 'function', function: { name: 'f', arguments: '{}' } }],
        },
        {
          id: '2',
          role: 'tool',
          content: [{ type: 'text', text: 'result' }],
          sessionId: 's1',
          threadId: 't1',
          tool_call_id: 'c1',
        },
      ];

      const result = removeInvalidToolUseAndToolResponse(messages);
      expect(result).toHaveLength(2);
    });
  });
});

import { describe, it, expect } from 'vitest';
import {
  sanitizeJsonField,
  sanitizeToolCall,
  sanitizeMessage,
} from '../sanitizer';
import { Message, ToolCall } from '@/models/chat';

describe('AI Service Sanitizer', () => {
  describe('sanitizeJsonField', () => {
    it('should return valid JSON string as is', () => {
      const json = '{"key":"value"}';
      expect(sanitizeJsonField(json)).toBe(json);
    });

    it('should stringify invalid JSON string', () => {
      const invalidJson = '{key:value}';
      const expected = JSON.stringify(invalidJson);
      expect(sanitizeJsonField(invalidJson)).toBe(expected);
    });
  });

  describe('sanitizeToolCall', () => {
    it('should sanitize tool call arguments', () => {
      const toolCall: ToolCall = {
        id: 'call_1',
        type: 'function',
        function: {
          name: 'testTool',
          arguments: '{invalid}',
        },
      };

      const sanitized = sanitizeToolCall(toolCall);
      expect(sanitized.function.arguments).toBe(JSON.stringify('{invalid}'));
    });

    it('should keep valid tool call arguments as is', () => {
      const toolCall: ToolCall = {
        id: 'call_1',
        type: 'function',
        function: {
          name: 'testTool',
          arguments: '{"valid":true}',
        },
      };

      const sanitized = sanitizeToolCall(toolCall);
      expect(sanitized.function.arguments).toBe('{"valid":true}');
    });
  });

  describe('sanitizeMessage', () => {
    it('should sanitize tool_calls in message', () => {
      const message: Message = {
        id: 'msg_1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: {
              name: 'testTool',
              arguments: '{invalid}',
            },
          },
        ],
        sessionId: 'session_1',
        createdAt: new Date(),
      };

      const sanitized = sanitizeMessage(message);
      expect(sanitized.tool_calls![0].function.arguments).toBe(
        JSON.stringify('{invalid}'),
      );
    });

    it('should sanitize thinking content', () => {
      const message: Message = {
        id: 'msg_1',
        role: 'assistant',
        content: [],
        thinking: '{invalid thinking}',
        sessionId: 'session_1',
        createdAt: new Date(),
      };

      const sanitized = sanitizeMessage(message);
      expect(sanitized.thinking).toBe(JSON.stringify('{invalid thinking}'));
    });

    it('should handle message without tool_calls or thinking', () => {
      const message: Message = {
        id: 'msg_1',
        role: 'user',
        content: [],
        sessionId: 'session_1',
        createdAt: new Date(),
      };

      const sanitized = sanitizeMessage(message);
      expect(sanitized).toEqual(message);
    });
  });
});

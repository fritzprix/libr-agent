import { describe, it, expect } from 'vitest';
import {
  hasToolCallError,
  hasUIResource,
  parseToolName,
  parseToolArguments,
  formatExecutionTime,
  formatToolArgumentsSummary,
} from '../tool-call-utils';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';

describe('tool-call-utils', () => {
  describe('hasToolCallError', () => {
    it('should return true if toolResult.error is present', () => {
      const message = {
        error: {
          displayMessage: 'Error',
          type: 'TOOL_EXECUTION_ERROR',
          recoverable: false,
        },
      } as unknown as Message;
      expect(hasToolCallError(message)).toBe(true);
    });

    it('should return true if toolResult.content contains an item with isError: true', () => {
      const message = {
        content: [
          { type: 'text', text: 'Some text' },
          { type: 'text', text: 'Error', isError: true },
        ],
      } as unknown as Message;
      expect(hasToolCallError(message)).toBe(true);
    });

    it('should return false if neither error property nor error content exists', () => {
      const message = {
        content: [{ type: 'text', text: 'Success' }],
      } as unknown as Message;
      expect(hasToolCallError(message)).toBe(false);
    });

    it('should return false for undefined message', () => {
      expect(hasToolCallError(undefined)).toBe(false);
    });
  });

  describe('hasUIResource', () => {
    it("should return true if content has type 'resource' and mimeType is present", () => {
      const message = {
        content: [
          {
            type: 'resource',
            resource: { mimeType: 'image/png', uri: 'file://test.png' },
          },
        ],
      } as unknown as Message;
      expect(hasUIResource(message)).toBe(true);
    });

    it("should return false if content has type 'resource' but no mimeType", () => {
      const message = {
        content: [
          {
            type: 'resource',
            resource: { uri: 'file://test.png' },
          },
        ],
      } as unknown as Message;
      expect(hasUIResource(message)).toBe(false);
    });

    it("should return false if content does not have type 'resource'", () => {
      const message = {
        content: [{ type: 'text', text: 'Hello' }],
      } as unknown as Message;
      expect(hasUIResource(message)).toBe(false);
    });

    it('should return false for undefined message', () => {
      expect(hasUIResource(undefined)).toBe(false);
    });
  });

  describe('parseToolName', () => {
    it('should extract tool name from builtin_server__toolName', () => {
      expect(parseToolName('builtin_server__toolName')).toBe('toolName');
    });

    it('should return full name if no prefix/delimiter', () => {
      expect(parseToolName('toolName')).toBe('toolName');
    });

    it('should handle empty string', () => {
      expect(parseToolName('')).toBe('');
    });

    it('should handle multiple underscores correctly (take last part)', () => {
      expect(parseToolName('a__b__c')).toBe('c');
    });
  });

  describe('parseToolArguments', () => {
    it('should parse valid JSON object strings', () => {
      const json = '{"key": "value", "num": 123}';
      expect(parseToolArguments(json)).toEqual({ key: 'value', num: 123 });
    });

    it('should wrap non-object JSON values (string)', () => {
      const json = '"some string"';
      expect(parseToolArguments(json)).toEqual({ value: 'some string' });
    });

    it('should wrap non-object JSON values (number)', () => {
      const json = '123';
      expect(parseToolArguments(json)).toEqual({ value: 123 });
    });

    it('should wrap non-object JSON values (array)', () => {
      const json = '[1, 2, 3]';
      expect(parseToolArguments(json)).toEqual({ value: [1, 2, 3] });
    });

    it('should wrap non-object JSON values (null)', () => {
      const json = 'null';
      expect(parseToolArguments(json)).toEqual({ value: null });
    });

    it('should return { raw: string } for invalid JSON', () => {
      const invalidJson = '{ key: "value" }'; // Invalid JSON (missing quotes on key)
      expect(parseToolArguments(invalidJson)).toEqual({ raw: invalidJson });
    });
  });

  describe('formatExecutionTime', () => {
    it('should format < 1000ms as Xms', () => {
      expect(formatExecutionTime(500)).toBe('500ms');
      expect(formatExecutionTime(0)).toBe('0ms');
      expect(formatExecutionTime(999)).toBe('999ms');
    });

    it('should format >= 1000ms as X.Xs', () => {
      expect(formatExecutionTime(1000)).toBe('1.0s');
      expect(formatExecutionTime(1500)).toBe('1.5s');
      expect(formatExecutionTime(2345)).toBe('2.3s');
    });
  });

  describe('formatToolArgumentsSummary', () => {
    it('should format simple objects', () => {
      const args = { key1: 'value1', key2: 123 };
      expect(formatToolArgumentsSummary(args)).toBe('key1: value1, key2: 123');
    });

    it('should handle JSON stringification for object values', () => {
      const args = { key: { nested: true } };
      expect(formatToolArgumentsSummary(args)).toBe('key: {"nested":true}');
    });

    it('should truncate long summaries', () => {
      const args = {
        longKey: 'This is a very long string that should be truncated',
        anotherKey: 'another value',
      };
      const result = formatToolArgumentsSummary(args, 20);
      expect(result.endsWith('...')).toBe(true);
      expect(result.length).toBeLessThanOrEqual(23); // 20 + 3 dots
    });

    it('should return empty string for empty/null args', () => {
      expect(formatToolArgumentsSummary({})).toBe('');
      // @ts-expect-error - testing invalid input
      expect(formatToolArgumentsSummary(null)).toBe('');
      // @ts-expect-error - testing invalid input
      expect(formatToolArgumentsSummary(undefined)).toBe('');
    });
  });
});

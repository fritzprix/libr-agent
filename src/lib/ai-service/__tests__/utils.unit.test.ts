import { describe, it, expect } from 'vitest';
import {
  isAIServiceProvider,
  tryParse,
  safeJsonStringify,
  formatToolCall,
  generateToolCallId,
  calculateTokensPerSecond,
  formatUsageMetrics,
  processMessageContent,
  processMultiModalContent,
} from '../utils';
import { formatNumber } from '@/lib/utils';
import { AIServiceProvider, TokenUsage } from '../types';
import { MCPContent } from '@/lib/mcp';

describe('AI Service Utils', () => {
  describe('isAIServiceProvider', () => {
    it('should return true for valid provider', () => {
      expect(isAIServiceProvider(AIServiceProvider.OpenAI)).toBe(true);
    });

    it('should return false for invalid provider', () => {
      expect(isAIServiceProvider('invalid_provider')).toBe(false);
    });
  });

  describe('tryParse', () => {
    it('should return parsed object for valid JSON', () => {
      expect(tryParse('{"key":"value"}')).toEqual({ key: 'value' });
    });

    it('should return undefined for invalid JSON', () => {
      expect(tryParse('{invalid}')).toBeUndefined();
    });

    it('should return undefined for undefined input', () => {
      expect(tryParse(undefined)).toBeUndefined();
    });
  });

  describe('safeJsonStringify', () => {
    it('should stringify object', () => {
      expect(safeJsonStringify({ key: 'value' })).toBe('{"key":"value"}');
    });

    it('should return empty object string for null/undefined', () => {
      expect(safeJsonStringify(null)).toBe('{}');
      expect(safeJsonStringify(undefined)).toBe('{}');
    });

    it('should handle circular reference (mocking JSON.stringify failure)', () => {
      // It's hard to force JSON.stringify to fail in JS without circular structure
      // but safeJsonStringify wraps in try/catch and returns '{}' on failure.
      const circular: Record<string, unknown> = {};
      circular.myself = circular;
      expect(safeJsonStringify(circular)).toBe('{}');
    });
  });

  describe('formatToolCall', () => {
    it('should format tool call correctly', () => {
      const id = 'call_1';
      const name = 'testTool';
      const args = { key: 'value' };
      const result = formatToolCall(id, name, args);

      expect(result).toEqual({
        id,
        function: {
          name,
          arguments: '{"key":"value"}',
        },
      });
    });
  });

  describe('generateToolCallId', () => {
    it('should generate id starting with tool_', () => {
      expect(generateToolCallId().startsWith('tool_')).toBe(true);
    });
  });

  describe('calculateTokensPerSecond', () => {
    it('should calculate correct rate', () => {
      const usage: TokenUsage = {
        promptTokens: 10,
        completionTokens: 100,
        totalTokens: 110,
      };
      const durationMs = 2000;
      expect(calculateTokensPerSecond(usage, durationMs)).toBe(50); // 100 / 2 * 1000 = 50
    });

    it('should return 0 if duration is 0', () => {
      const usage: TokenUsage = {
        promptTokens: 10,
        completionTokens: 100,
        totalTokens: 110,
      };
      expect(calculateTokensPerSecond(usage, 0)).toBe(0);
    });

    it('should return 0 if completion tokens is 0', () => {
      const usage: TokenUsage = {
        promptTokens: 10,
        completionTokens: 0,
        totalTokens: 10,
      };
      expect(calculateTokensPerSecond(usage, 2000)).toBe(0);
    });
  });

  describe('formatUsageMetrics', () => {
    it('should format metrics with locale strings', () => {
      const usage: TokenUsage = {
        promptTokens: 1000,
        completionTokens: 2000,
        totalTokens: 3000,
      };
      const result = formatUsageMetrics(usage);
      expect(result.input).toBe(formatNumber(usage.promptTokens));
      expect(result.output).toBe(formatNumber(usage.completionTokens));
      expect(result.total).toBe(formatNumber(usage.totalTokens));
      expect(result.speed).toBeUndefined();
    });

    it('should include speed if evalDuration is present', () => {
      const usage: TokenUsage = {
        promptTokens: 10,
        completionTokens: 100,
        totalTokens: 110,
        details: { evalDuration: 2000 },
      };
      const result = formatUsageMetrics(usage);
      expect(result.speed).toBe('50.0 t/s');
    });
  });

  describe('processMessageContent', () => {
    it('should return string as is', () => {
      expect(processMessageContent('hello')).toBe('hello');
    });

    it('should extract text from MCPContent array', () => {
      const content: MCPContent[] = [
        { type: 'text', text: 'Hello ' },
        { type: 'image', data: '', mimeType: '' },
        { type: 'text', text: 'World' },
      ];
      expect(processMessageContent(content)).toBe('Hello \nWorld');
    });

    it('should return empty string for non-array/non-string', () => {
        // @ts-expect-error - Testing invalid input
      expect(processMessageContent(123)).toBe('');
    });
  });

  describe('processMultiModalContent', () => {
    it('should process text content', () => {
      const content: MCPContent[] = [{ type: 'text', text: 'Hello' }];
      expect(processMultiModalContent(content)).toEqual([
        { type: 'text', text: 'Hello' },
      ]);
    });

    it('should process image content', () => {
      const content: MCPContent[] = [
        { type: 'image', data: 'base64', mimeType: 'image/png' },
      ];
      expect(processMultiModalContent(content)).toEqual([
        { type: 'image', image: 'base64', mimeType: 'image/png' },
      ]);
    });

    it('should process audio content', () => {
      const content: MCPContent[] = [
        { type: 'audio', data: 'base64', mimeType: 'audio/mp3' },
      ];
      expect(processMultiModalContent(content)).toEqual([
        { type: 'audio', audio: 'base64', mimeType: 'audio/mp3' },
      ]);
    });

    it('should degrade unresolved uri-only media into text instead of fake base64', () => {
      const content: MCPContent[] = [
        { type: 'image', uri: 'file:///tmp/example.png', mimeType: 'image/png' },
      ];
      expect(processMultiModalContent(content)).toEqual([
        {
          type: 'text',
          text: '[unresolved image omitted from multimodal request: file:///tmp/example.png]',
        },
      ]);
    });

    it('should handle unknown content types', () => {
      const content: MCPContent[] = [
        // @ts-expect-error - Testing unknown type
        { type: 'unknown' },
      ];
      expect(processMultiModalContent(content)).toEqual([
        { type: 'text', text: '[unknown]' },
      ]);
    });
  });
});

import { describe, it, expect } from 'vitest';
import {
  toToolResult,
  hasStructuredData,
  hasTextContent,
  type ToolResult,
  type MCPResultEnvelope,
} from './tool-result';

describe('ToolResult utilities', () => {
  describe('toToolResult', () => {
    it('should extract structured content when present', () => {
      const mcpResponse = {
        result: {
          structuredContent: { storeId: 'store_123', createdAt: '2024-01-01' },
          content: [{ type: 'text', text: 'Store created successfully' }],
        },
      };

      const result = toToolResult<{ storeId: string; createdAt: string }>(mcpResponse);

      expect(result.data).toEqual({ storeId: 'store_123', createdAt: '2024-01-01' });
      expect(result.text).toBe('Store created successfully');
      expect(result.raw).toEqual(mcpResponse.result);
    });

    it('should extract text content when present', () => {
      const mcpResponse = {
        result: {
          content: [{ type: 'text', text: 'Operation completed' }],
        },
      };

      const result = toToolResult(mcpResponse);

      expect(result.data).toBeUndefined();
      expect(result.text).toBe('Operation completed');
      expect(result.raw).toEqual(mcpResponse.result);
    });

    it('should handle text content with non-text type', () => {
      const mcpResponse = {
        result: {
          content: [{ type: 'image', text: 'base64data...' }],
        },
      };

      const result = toToolResult(mcpResponse);

      expect(result.data).toBeUndefined();
      expect(result.text).toBeUndefined();
      expect(result.raw).toEqual(mcpResponse.result);
    });

    it('should handle empty content array', () => {
      const mcpResponse = {
        result: {
          content: [],
        },
      };

      const result = toToolResult(mcpResponse);

      expect(result.data).toBeUndefined();
      expect(result.text).toBeUndefined();
      expect(result.raw).toEqual(mcpResponse.result);
    });

    it('should handle missing result property', () => {
      const mcpResponse = {};

      const result = toToolResult(mcpResponse);

      expect(result.data).toBeUndefined();
      expect(result.text).toBeUndefined();
      expect(result.raw).toEqual({});
    });

    it('should handle null/undefined responses', () => {
      const result1 = toToolResult(null);
      const result2 = toToolResult(undefined);

      expect(result1.data).toBeUndefined();
      expect(result1.text).toBeUndefined();
      expect(result1.raw).toEqual({});

      expect(result2.data).toBeUndefined();
      expect(result2.text).toBeUndefined();
      expect(result2.raw).toEqual({});
    });

    it('should prefer structured content over text when both present', () => {
      const mcpResponse = {
        result: {
          structuredContent: { id: 'structured_123' },
          content: [{ type: 'text', text: 'Text content' }],
        },
      };

      const result = toToolResult<{ id: string }>(mcpResponse);

      expect(result.data).toEqual({ id: 'structured_123' });
      expect(result.text).toBe('Text content');
      expect(result.raw).toEqual(mcpResponse.result);
    });

    it('should handle complex structured content types', () => {
      interface ComplexOutput {
        storeId: string;
        files: Array<{ name: string; size: number }>;
        metadata: { sessionId: string; timestamp: number };
      }

      const structuredContent: ComplexOutput = {
        storeId: 'store_complex',
        files: [
          { name: 'file1.txt', size: 1024 },
          { name: 'file2.json', size: 512 },
        ],
        metadata: { sessionId: 'session_123', timestamp: Date.now() },
      };

      const mcpResponse = {
        result: {
          structuredContent,
          content: [{ type: 'text', text: 'Files processed successfully' }],
        },
      };

      const result = toToolResult<ComplexOutput>(mcpResponse);

      expect(result.data).toEqual(structuredContent);
      expect(result.text).toBe('Files processed successfully');
      expect(result.data?.files).toHaveLength(2);
      expect(result.data?.metadata.sessionId).toBe('session_123');
    });
  });

  describe('hasStructuredData', () => {
    it('should return true when data is present', () => {
      const result: ToolResult<{ id: string }> = {
        data: { id: 'test_123' },
        text: 'Test message',
        raw: {},
      };

      expect(hasStructuredData(result)).toBe(true);
      
      // Type narrowing should work
      if (hasStructuredData(result)) {
        expect(result.data.id).toBe('test_123');
      }
    });

    it('should return false when data is undefined', () => {
      const result: ToolResult<{ id: string }> = {
        data: undefined,
        text: 'Test message',
        raw: {},
      };

      expect(hasStructuredData(result)).toBe(false);
    });

    it('should return false when data is null', () => {
      const result: ToolResult<{ id: string }> = {
        data: null as unknown as { id: string },
        text: 'Test message',
        raw: {},
      };

      expect(hasStructuredData(result)).toBe(false);
    });
  });

  describe('hasTextContent', () => {
    it('should return true when text is present and non-empty', () => {
      const result: ToolResult = {
        data: undefined,
        text: 'Test message',
        raw: {},
      };

      expect(hasTextContent(result)).toBe(true);
      
      // Type narrowing should work
      if (hasTextContent(result)) {
        expect(result.text).toBe('Test message');
      }
    });

    it('should return false when text is undefined', () => {
      const result: ToolResult = {
        data: { id: 'test' },
        text: undefined,
        raw: {},
      };

      expect(hasTextContent(result)).toBe(false);
    });

    it('should return false when text is empty string', () => {
      const result: ToolResult = {
        data: { id: 'test' },
        text: '',
        raw: {},
      };

      expect(hasTextContent(result)).toBe(false);
    });

    it('should return false when text is not a string', () => {
      const result: ToolResult = {
        data: { id: 'test' },
        text: null as unknown as string,
        raw: {},
      };

      expect(hasTextContent(result)).toBe(false);
    });
  });

  describe('MCPResultEnvelope type', () => {
    it('should accept valid envelope structures', () => {
      const envelope1: MCPResultEnvelope = {
        structuredContent: { key: 'value' },
      };

      const envelope2: MCPResultEnvelope = {
        content: [{ type: 'text', text: 'hello' }],
      };

      const envelope3: MCPResultEnvelope = {
        content: [{ type: 'text', text: 'hello' }],
        structuredContent: { data: 'structured' },
      };

      const envelope4: MCPResultEnvelope = {};

      expect(envelope1).toBeDefined();
      expect(envelope2).toBeDefined();
      expect(envelope3).toBeDefined();
      expect(envelope4).toBeDefined();
    });
  });
});
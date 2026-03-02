import { describe, it, expect } from 'vitest';
import {
  isMCPSuccess,
  isMCPError,
  isValidMCPResult,
  extractStructuredContent,
  hasStructuredContent,
  isExtendedResponse,
} from '../type-guards';
import { MCPResponse, MCPResult, ExtendedMCPResponse } from '../../protocol';

const baseResponse = { jsonrpc: '2.0' as const, id: null };

describe('MCP Type Guards', () => {
  describe('isMCPSuccess', () => {
    it('should return true for a success response', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        result: {
          content: [],
        },
      };
      expect(isMCPSuccess(response)).toBe(true);
    });

    it('should return false if error is present', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        error: { code: 1, message: 'Error' },
      };
      expect(isMCPSuccess(response)).toBe(false);
    });

    it('should return false if result is missing', () => {
      const response: MCPResponse<unknown> = {
          ...baseResponse,
      };
      expect(isMCPSuccess(response)).toBe(false);
    });
  });

  describe('isMCPError', () => {
    it('should return true for an error response', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        error: { code: 1, message: 'Error' },
      };
      expect(isMCPError(response)).toBe(true);
    });

    it('should return false if error is missing', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        result: {},
      };
      expect(isMCPError(response)).toBe(false);
    });
  });

  describe('isValidMCPResult', () => {
    it('should return true if content array is non-empty', () => {
      const result: MCPResult = {
        content: [{ type: 'text', text: 'Hello' }],
      };
      expect(isValidMCPResult(result)).toBe(true);
    });

    it('should return true if structuredContent exists', () => {
      const result: MCPResult = {
        structuredContent: { data: 'test' },
      };
      expect(isValidMCPResult(result)).toBe(true);
    });

    it('should return false if both are empty/missing', () => {
      const result: MCPResult = {
        content: [],
      };
      expect(isValidMCPResult(result)).toBe(false);
    });

    it('should return false if content is missing and structuredContent is missing', () => {
        const result: MCPResult = {};
        expect(isValidMCPResult(result)).toBe(false);
    });
  });

  describe('extractStructuredContent', () => {
    it('should return structuredContent from valid success response', () => {
      const data = { key: 'value' };
      const response: MCPResponse<typeof data> = {
        ...baseResponse,
        result: {
          structuredContent: data,
        },
      };
      expect(extractStructuredContent(response)).toEqual(data);
    });

    it('should return null for error response', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        error: { code: 1, message: 'Error' },
      };
      expect(extractStructuredContent(response)).toBeNull();
    });

    it('should return null if result is missing', () => {
      const response: MCPResponse<unknown> = {
          ...baseResponse,
      };
      expect(extractStructuredContent(response)).toBeNull();
    });

    it('should return null if result contains sampling', () => {
      const response: MCPResponse<unknown> = {
          ...baseResponse,
        result: {
          // @ts-expect-error - Manually constructing sampling result for test
          sampling: 'test',
        },
      };
      expect(extractStructuredContent(response)).toBeNull();
    });

    it('should return null if result is empty', () => {
        const response: MCPResponse<unknown> = {
            ...baseResponse,
            result: {}
        };
        expect(extractStructuredContent(response)).toBeNull();
    });
  });

  describe('hasStructuredContent', () => {
    it('should return true if response has structuredContent', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        result: {
          structuredContent: {},
        },
      };
      expect(hasStructuredContent(response)).toBe(true);
    });

    it('should return false if response does not have structuredContent', () => {
      const response: MCPResponse<unknown> = {
        ...baseResponse,
        result: {
          content: [],
        },
      };
      expect(hasStructuredContent(response)).toBe(false);
    });
  });

  describe('isExtendedResponse', () => {
    it('should return true if serviceInfo is present', () => {
      const response = {
        ...baseResponse,
        serviceInfo: {
          serverName: 'test',
          toolName: 'test',
          backendType: 'ExternalMCP',
        },
      };
      expect(isExtendedResponse(response as ExtendedMCPResponse)).toBe(true);
    });

    it('should return false if serviceInfo is missing', () => {
      const response = {
        ...baseResponse,
        result: {},
      };
      expect(isExtendedResponse(response as ExtendedMCPResponse)).toBe(false);
    });

    it('should return false if response is null', () => {
        expect(isExtendedResponse(null as unknown as ExtendedMCPResponse)).toBe(false);
    });
  });
});

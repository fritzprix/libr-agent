import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { WebMCPProvider, useWebMCP } from './WebMCPContext';
import { WebMCPProxy } from '@/lib/web-mcp/mcp-proxy';

// Mock the WebMCPProxy
vi.mock('@/lib/web-mcp/mcp-proxy');
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
  }),
}));

// Mock the MCP Worker
vi.mock('@/lib/web-mcp/mcp-worker.ts?worker', () => {
  return {
    default: vi.fn(() => ({
      postMessage: vi.fn(),
      terminate: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  };
});

const MockedWebMCPProxy = vi.mocked(WebMCPProxy);

describe('WebMCPContext Response Parsing', () => {
  let mockProxy: {
    initialize: ReturnType<typeof vi.fn>;
    loadServer: ReturnType<typeof vi.fn>;
    listTools: ReturnType<typeof vi.fn>;
    callTool: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    
    mockProxy = {
      initialize: vi.fn().mockResolvedValue(undefined),
      loadServer: vi.fn().mockResolvedValue(undefined),
      listTools: vi.fn().mockResolvedValue([
        { name: 'test-server__testTool', description: 'A test tool' },
      ]),
      callTool: vi.fn(),
    };

    MockedWebMCPProxy.mockImplementation(() => mockProxy as unknown as WebMCPProxy);
  });

  const wrapper = ({ children }: { children: React.ReactNode }) => {
    return <WebMCPProvider>{children}</WebMCPProvider>;
  };

  describe('Response parsing precedence', () => {
    it('should return structuredContent when available', async () => {
      const structuredResponse = {
        result: {
          structuredContent: { storeId: 'store_123', createdAt: '2024-01-01' },
          content: [{ type: 'text', text: 'Store created' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(structuredResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({ storeId: 'store_123', createdAt: '2024-01-01' });
      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', undefined);
    });

    it('should parse JSON from text content when structuredContent is missing', async () => {
      const jsonTextResponse = {
        result: {
          content: [{ type: 'text', text: '{"message":"success","id":42}' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(jsonTextResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({ message: 'success', id: 42 });
    });

    it('should return raw text when JSON parsing fails', async () => {
      const plainTextResponse = {
        result: {
          content: [{ type: 'text', text: 'Plain text response that is not JSON' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(plainTextResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toBe('Plain text response that is not JSON');
    });

    it('should return raw result object as fallback', async () => {
      const rawResponse = {
        result: {
          custom: 'data',
          numbers: [1, 2, 3],
        },
      };

      mockProxy.callTool.mockResolvedValue(rawResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({ custom: 'data', numbers: [1, 2, 3] });
    });

    it('should handle responses with no content', async () => {
      const emptyResponse = {
        result: {},
      };

      mockProxy.callTool.mockResolvedValue(emptyResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({});
    });

    it('should prefer structuredContent over text even when both are present', async () => {
      const bothPresentResponse = {
        result: {
          structuredContent: { priority: 'structured', type: 'data' },
          content: [{ type: 'text', text: '{"priority":"text","type":"fallback"}' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(bothPresentResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({ priority: 'structured', type: 'data' });
    });

    it('should handle non-text content types', async () => {
      const imageResponse = {
        result: {
          content: [{ type: 'image', text: 'base64data...' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(imageResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toEqual({ content: [{ type: 'image', text: 'base64data...' }] });
    });

    it('should handle malformed JSON gracefully', async () => {
      const malformedJsonResponse = {
        result: {
          content: [{ type: 'text', text: '{"incomplete": json' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(malformedJsonResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toBe('{"incomplete": json');
    });
  });

  describe('Error handling', () => {
    it('should throw error when response contains error field', async () => {
      const errorResponse = {
        error: {
          message: 'Tool execution failed',
          code: -1,
        },
      };

      mockProxy.callTool.mockResolvedValue(errorResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      
      await expect((serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool())
        .rejects.toThrow('Tool execution failed');
    });

    it('should handle undefined structuredContent correctly', async () => {
      const undefinedStructuredResponse = {
        result: {
          structuredContent: undefined,
          content: [{ type: 'text', text: 'fallback to text' }],
        },
      };

      mockProxy.callTool.mockResolvedValue(undefinedStructuredResponse);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      const toolResult = await (serverProxy as unknown as { testTool: () => Promise<unknown> }).testTool();

      expect(toolResult).toBe('fallback to text');
    });
  });

  describe('Tool method name normalization', () => {
    it('should remove server prefix from tool names', async () => {
      mockProxy.listTools.mockResolvedValue([
        { name: 'content-store__createStore', description: 'Create store' },
        { name: 'content-store__addContent', description: 'Add content' },
        { name: 'standalone_tool', description: 'Standalone tool' },
      ]);

      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('content-store');

      expect(serverProxy.tools).toHaveLength(3);
      expect('createStore' in serverProxy).toBe(true);
      expect('addContent' in serverProxy).toBe(true);
      expect('standalone_tool' in serverProxy).toBe(true);
    });
  });

  describe('Argument handling', () => {
    beforeEach(() => {
      mockProxy.callTool.mockResolvedValue({ result: { structuredContent: 'success' } });
    });

    it('should handle undefined arguments', async () => {
      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      await (serverProxy as unknown as { testTool: (args?: unknown) => Promise<unknown> }).testTool();

      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', undefined);
    });

    it('should handle object arguments', async () => {
      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      await (serverProxy as unknown as { testTool: (args: unknown) => Promise<unknown> }).testTool({ key: 'value' });

      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', { key: 'value' });
    });

    it('should parse JSON string arguments', async () => {
      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      await (serverProxy as unknown as { testTool: (args: unknown) => Promise<unknown> }).testTool('{"key":"value"}');

      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', { key: 'value' });
    });

    it('should handle non-JSON string arguments', async () => {
      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      await (serverProxy as unknown as { testTool: (args: unknown) => Promise<unknown> }).testTool('plain text');

      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', { raw: 'plain text' });
    });

    it('should handle other argument types', async () => {
      const { result } = renderHook(() => useWebMCP(), { wrapper });

      await waitFor(() => expect(result.current.initialized).toBe(true));

      const serverProxy = await result.current.getServerProxy('test-server');
      await (serverProxy as unknown as { testTool: (args: unknown) => Promise<unknown> }).testTool(42);

      expect(mockProxy.callTool).toHaveBeenCalledWith('test-server', 'testTool', { value: 42 });
    });
  });
});
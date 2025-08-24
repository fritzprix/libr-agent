import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { ReactNode } from 'react';
import { BrowserToolProvider } from './BrowserToolProvider';
import { BuiltInToolProvider, useBuiltInTool } from './index';
import { ToolCall } from '@/models/chat';

// Mock dependencies
vi.mock('@/hooks/use-browser-invoker', () => ({
  useBrowserInvoker: () => ({
    executeScript: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    writeFile: vi.fn(),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('turndown', () => {
  return {
    default: function TurndownService() {
      return {
        addRule: vi.fn(),
        turndown: vi.fn().mockReturnValue('# Converted Markdown'),
      };
    },
  };
});

const TestWrapper = ({ children }: { children: ReactNode }) => (
  <BuiltInToolProvider>
    <BrowserToolProvider />
    {children}
  </BuiltInToolProvider>
);

describe('BrowserToolProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('browser tool registration', () => {
    it('should register browser tools with correct service ID', () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const browserTools = result.current.availableTools.filter(tool =>
        tool.name.startsWith('builtin.browser__')
      );

      expect(browserTools.length).toBeGreaterThan(0);
      expect(browserTools.some(tool => tool.name === 'builtin.browser__browser_createSession')).toBe(true);
      expect(browserTools.some(tool => tool.name === 'builtin.browser__browser_elementExists')).toBe(true);
    });

    it('should handle browser tool execution with proper argument parsing', async () => {
      const mockExecuteScript = vi.fn().mockResolvedValue('true');
      
      vi.doMock('@/hooks/use-browser-invoker', () => ({
        useBrowserInvoker: () => ({
          executeScript: mockExecuteScript,
        }),
      }));

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      // Find the elementExists tool
      const elementExistsTool = result.current.availableTools.find(
        tool => tool.name === 'builtin.browser__browser_elementExists'
      );

      expect(elementExistsTool).toBeDefined();
    });
  });

  describe('browser_elementExists tool edge cases', () => {
    const mockExecuteScript = vi.fn();

    beforeEach(() => {
      vi.doMock('@/hooks/use-browser-invoker', () => ({
        useBrowserInvoker: () => ({
          executeScript: mockExecuteScript,
        }),
      }));
    });

    it('should handle executeScript returning boolean true', async () => {
      mockExecuteScript.mockResolvedValue('true');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element'
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      expect(response.result?.content?.[0]).toEqual({ type: 'text', text: 'true' });
    });

    it('should handle executeScript returning boolean false', async () => {
      mockExecuteScript.mockResolvedValue('false');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-2',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element'
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      expect(response.result?.content?.[0]).toEqual({ type: 'text', text: 'false' });
    });

    it('should handle executeScript returning JSON string with boolean', async () => {
      mockExecuteScript.mockResolvedValue(JSON.stringify(true));

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-3',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element'
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      // Should detect 'true' in the JSON string
      expect(response.result?.content?.[0]).toEqual({ type: 'text', text: 'true' });
    });

    it('should handle executeScript returning plain string without boolean', async () => {
      mockExecuteScript.mockResolvedValue('element found successfully');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-4',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element'
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      // Should default to 'false' when no 'true' is found
      expect(response.result?.content?.[0]).toEqual({ type: 'text', text: 'false' });
    });
  });

  describe('argument parsing edge cases', () => {
    it('should handle empty string arguments', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-5',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '',
        },
      };

      // Should not throw error for empty arguments
      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });

    it('should handle invalid JSON arguments gracefully', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-6',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '{invalid json}',
        },
      };

      // Should not throw error for invalid JSON
      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });

    it('should handle object arguments', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-7',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '{"someKey": "someValue"}',
        },
      };

      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });
  });

  describe('tool not found error handling', () => {
    it('should throw error for non-existent browser tool', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-8',
        type: 'function',
        function: {
          name: 'builtin.browser__nonexistent_tool',
          arguments: '{}',
        },
      };

      await expect(result.current.executeTool(toolCall))
        .rejects.toThrow('Browser tool not found: nonexistent_tool');
    });
  });
});
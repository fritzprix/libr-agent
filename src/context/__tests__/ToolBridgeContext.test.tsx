import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ToolBridgeProvider, useToolBridge } from '../ToolBridgeContext';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import type { ToolCall } from '@/models/chat';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock useUnifiedMCP
vi.mock('@/hooks/use-unified-mcp', () => ({
  useUnifiedMCP: vi.fn(),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('ToolBridgeContext', () => {
  const mockUnlisten = vi.fn();
  const mockExecuteToolCall = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Setup listen mock
    (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);

    // Setup useUnifiedMCP mock
    (useUnifiedMCP as ReturnType<typeof vi.fn>).mockReturnValue({
      executeToolCall: mockExecuteToolCall,
    });
  });

  describe('Provider Setup', () => {
    it('should provide context value', () => {
      const { result } = renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      expect(result.current).toBeDefined();
      expect(result.current).toEqual({});
    });

    it('should throw error when used outside provider', () => {
      // Suppress console.error for this test
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useToolBridge());
      }).toThrow('useToolBridge must be used within ToolBridgeProvider');

      console.error = originalError;
    });

    it('should register event listener on mount', async () => {
      renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalledWith(
          'tool:execute-request',
          expect.any(Function),
        );
      });
    });

    it('should cleanup on unmount', async () => {
      const { unmount } = renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalled();
      });

      unmount();

      await waitFor(() => {
        expect(mockUnlisten).toHaveBeenCalled();
      });
    });
  });

  describe('Tool Execution', () => {
    it('should execute tool call and report result', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'tool:execute-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockExecuteToolCall.mockResolvedValue({
        jsonrpc: '2.0',
        id: 'call1',
        result: {
          content: [{ type: 'text', text: 'Tool result content' }],
        },
      });

      renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      const toolCall: ToolCall = {
        id: 'call1',
        type: 'function',
        function: {
          name: 'test_tool',
          arguments: '{"arg": "value"}',
        },
      };

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          toolCall,
        },
      });

      await waitFor(() => {
        expect(mockExecuteToolCall).toHaveBeenCalledWith(toolCall);
        expect(invoke).toHaveBeenCalledWith('agent_handle_tool_result', {
          sessionId: 'test-session',
          toolCallId: 'call1',
          result: {
            success: true,
            content: 'Tool result content',
            error: undefined,
            isError: false,
          },
        });
      });
    });

    it('should handle tool execution errors', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'tool:execute-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockExecuteToolCall.mockResolvedValue({
        jsonrpc: '2.0',
        id: 'call1',
        error: {
          code: -32600,
          message: 'Tool execution failed',
        },
      });

      renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      const toolCall: ToolCall = {
        id: 'call1',
        type: 'function',
        function: {
          name: 'test_tool',
          arguments: '{"arg": "value"}',
        },
      };

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          toolCall,
        },
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('agent_handle_tool_result', {
          sessionId: 'test-session',
          toolCallId: 'call1',
          result: {
            success: false,
            content: '',
            error: expect.stringContaining('Tool execution failed'),
            isError: true,
          },
        });
      });
    });

    it('should handle exceptions during tool execution', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'tool:execute-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockExecuteToolCall.mockRejectedValue(new Error('Unexpected error'));

      renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      const toolCall: ToolCall = {
        id: 'call1',
        type: 'function',
        function: {
          name: 'test_tool',
          arguments: '{"arg": "value"}',
        },
      };

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          toolCall,
        },
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('agent_handle_tool_result', {
          sessionId: 'test-session',
          toolCallId: 'call1',
          result: {
            success: false,
            error: 'Unexpected error',
            isError: true,
          },
        });
      });
    });

    it('should handle multiple content items in result', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'tool:execute-request') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      mockExecuteToolCall.mockResolvedValue({
        jsonrpc: '2.0',
        id: 'call1',
        result: {
          content: [
            { type: 'text', text: 'Part 1' },
            { type: 'text', text: 'Part 2' },
          ],
        },
      });

      renderHook(() => useToolBridge(), {
        wrapper: ({ children }) => (
          <ToolBridgeProvider>{children}</ToolBridgeProvider>
        ),
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      const toolCall: ToolCall = {
        id: 'call1',
        type: 'function',
        function: {
          name: 'test_tool',
          arguments: '{}',
        },
      };

      // Trigger event
      await eventHandler?.({
        payload: {
          sessionId: 'test-session',
          toolCall,
        },
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('agent_handle_tool_result', {
          sessionId: 'test-session',
          toolCallId: 'call1',
          result: {
            success: true,
            content: 'Part 1\nPart 2',
            error: undefined,
            isError: false,
          },
        });
      });
    });
  });
});

import { describe, it, expect, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { ReactNode } from 'react';
import { BuiltInToolProvider, useBuiltInTool, BuiltInService } from './index';
import { ToolCall } from '@/models/chat';

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const TestWrapper = ({ children }: { children: ReactNode }) => (
  <BuiltInToolProvider>{children}</BuiltInToolProvider>
);

describe('BuiltInToolProvider', () => {
  describe('name parsing', () => {
    it('should parse builtin tool names with single underscore separator', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const mockService: BuiltInService = {
        listTools: () => [
          {
            name: 'list_directory',
            description: 'List directory contents',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: vi.fn().mockResolvedValue({
          jsonrpc: '2.0',
          id: 'test',
          result: { content: [{ type: 'text', text: 'success' }] },
        }),
      };

      act(() => {
        result.current.register('filesystem', mockService);
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.filesystem__list_directory',
          arguments: '{}',
        },
      };

      await act(async () => {
        await result.current.executeTool(toolCall);
      });

      expect(mockService.executeTool).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'test-call',
          function: expect.objectContaining({
            name: 'list_directory',
            arguments: '{}',
          }),
        }),
      );
    });

    it('should parse builtin tool names with multiple underscores correctly', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const mockService: BuiltInService = {
        listTools: () => [
          {
            name: 'name__with__underscores',
            description: 'Tool with underscores',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: vi.fn().mockResolvedValue({
          jsonrpc: '2.0',
          id: 'test',
          result: { content: [{ type: 'text', text: 'success' }] },
        }),
      };

      act(() => {
        result.current.register('filesystem', mockService);
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.filesystem__name__with__underscores',
          arguments: '{}',
        },
      };

      await act(async () => {
        await result.current.executeTool(toolCall);
      });

      expect(mockService.executeTool).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'test-call',
          function: expect.objectContaining({
            name: 'name__with__underscores',
            arguments: '{}',
          }),
        }),
      );
    });

    it('should throw error for invalid builtin tool name format', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.invalidname',
          arguments: '{}',
        },
      };

      await expect(
        act(async () => {
          await result.current.executeTool(toolCall);
        }),
      ).rejects.toThrow('Invalid builtin tool name: invalidname');
    });

    it('should handle tool calls without builtin prefix', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const mockService: BuiltInService = {
        listTools: () => [
          {
            name: 'test_tool',
            description: 'Test tool',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: vi.fn().mockResolvedValue({
          jsonrpc: '2.0',
          id: 'test',
          result: { content: [{ type: 'text', text: 'success' }] },
        }),
      };

      act(() => {
        result.current.register('service', mockService);
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'service__test_tool',
          arguments: '{}',
        },
      };

      await act(async () => {
        await result.current.executeTool(toolCall);
      });

      expect(mockService.executeTool).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'test-call',
          function: expect.objectContaining({
            name: 'test_tool',
            arguments: '{}',
          }),
        }),
      );
    });
  });

  describe('service registration and routing', () => {
    it('should register and list tools correctly', () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const mockService: BuiltInService = {
        listTools: () => [
          {
            name: 'tool1',
            description: 'First tool',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
          {
            name: 'tool2',
            description: 'Second tool',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: vi.fn(),
      };

      act(() => {
        result.current.register('testservice', mockService);
      });

      expect(result.current.availableTools).toHaveLength(2);
      expect(result.current.availableTools[0].name).toBe(
        'builtin.testservice__tool1',
      );
      expect(result.current.availableTools[1].name).toBe(
        'builtin.testservice__tool2',
      );
    });

    it('should route tool calls to correct service', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const service1Mock = vi.fn().mockResolvedValue({
        jsonrpc: '2.0',
        id: 'test',
        result: { content: [{ type: 'text', text: 'service1 result' }] },
      });

      const service2Mock = vi.fn().mockResolvedValue({
        jsonrpc: '2.0',
        id: 'test',
        result: { content: [{ type: 'text', text: 'service2 result' }] },
      });

      const service1: BuiltInService = {
        listTools: () => [
          {
            name: 'tool1',
            description: 'Tool 1',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: service1Mock,
      };

      const service2: BuiltInService = {
        listTools: () => [
          {
            name: 'tool2',
            description: 'Tool 2',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: service2Mock,
      };

      act(() => {
        result.current.register('service1', service1);
        result.current.register('service2', service2);
      });

      // Test routing to service1
      const toolCall1: ToolCall = {
        id: 'call1',
        type: 'function',
        function: { name: 'builtin.service1__tool1', arguments: '{}' },
      };

      await act(async () => {
        await result.current.executeTool(toolCall1);
      });

      expect(service1Mock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'call1',
          function: expect.objectContaining({ name: 'tool1', arguments: '{}' }),
        }),
      );

      // Test routing to service2
      const toolCall2: ToolCall = {
        id: 'call2',
        type: 'function',
        function: { name: 'builtin.service2__tool2', arguments: '{}' },
      };

      await act(async () => {
        await result.current.executeTool(toolCall2);
      });

      expect(service2Mock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'call2',
          function: expect.objectContaining({ name: 'tool2', arguments: '{}' }),
        }),
      );
    });

    it('should throw error for unknown service', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.unknown__tool',
          arguments: '{}',
        },
      };

      await expect(
        act(async () => {
          await result.current.executeTool(toolCall);
        }),
      ).rejects.toThrow('No built-in service found for serviceId: unknown');
    });

    it('should unregister services correctly', () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const mockService: BuiltInService = {
        listTools: () => [
          {
            name: 'tool1',
            description: 'Tool 1',
            inputSchema: { type: 'object', properties: {}, required: [] },
          },
        ],
        executeTool: vi.fn(),
      };

      act(() => {
        result.current.register('testservice', mockService);
      });

      expect(result.current.availableTools).toHaveLength(1);

      act(() => {
        result.current.unregister('testservice');
      });

      expect(result.current.availableTools).toHaveLength(0);
    });
  });

  describe('service lifecycle', () => {
    it('should call loadService if provided during registration', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const loadServiceMock = vi.fn().mockResolvedValue(undefined);

      const mockService: BuiltInService = {
        listTools: () => [],
        executeTool: vi.fn(),
        loadService: loadServiceMock,
      };

      await act(async () => {
        result.current.register('testservice', mockService);
        // Wait a bit for async operations
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      expect(loadServiceMock).toHaveBeenCalled();
    });

    it('should call unloadService if provided during unregistration', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const unloadServiceMock = vi.fn().mockResolvedValue(undefined);

      const mockService: BuiltInService = {
        listTools: () => [],
        executeTool: vi.fn(),
        unloadService: unloadServiceMock,
      };

      act(() => {
        result.current.register('testservice', mockService);
      });

      await act(async () => {
        result.current.unregister('testservice');
        // Wait a bit for async operations
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      expect(unloadServiceMock).toHaveBeenCalled();
    });
  });
});

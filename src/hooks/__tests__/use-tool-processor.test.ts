import { renderHook, act, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useToolProcessor } from '../use-tool-processor';
import { Message } from '@/models/chat';

// Mock dependencies
const mockExecuteToolCall = vi.fn();
const mockSubmit = vi.fn();
const mockAddMessages = vi.fn();

vi.mock('@/context/SessionContext', () => ({
  useSessionContext: () => ({ current: { id: 'session-1' } }),
}));

vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: () => ({
    currentAssistant: { id: 'assistant-1', name: 'Test Agent' },
  }),
}));

vi.mock('@/hooks/use-unified-mcp', () => ({
  useUnifiedMCP: () => ({ executeToolCall: mockExecuteToolCall }),
}));

vi.mock('@/context/SessionHistoryContext', () => ({
  useSessionHistory: () => ({ addMessages: mockAddMessages }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('useToolProcessor Circuit Breaker', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockExecuteToolCall.mockResolvedValue({
      result: { content: [{ type: 'text', text: 'Success' }] },
    });
  });

  it('should trigger circuit breaker after 3 repetitive calls', async () => {
    const { result } = renderHook(() => useToolProcessor({ submit: mockSubmit }));

    const toolCall = {
      id: 'call-1',
      type: 'function' as const,
      function: { name: 'test_tool', arguments: '{"arg":"value"}' },
    };

    const message: Message = {
      id: 'msg-1',
      role: 'assistant',
      content: [],
      tool_calls: [toolCall],
      sessionId: 'session-1',
      threadId: 'thread-1',
    };

    // 1st call
    await act(async () => {
      result.current.processToolCalls(message);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalled());
    });

    // 2nd call
    const message2 = { ...message, id: 'msg-2' };
    await act(async () => {
      result.current.processToolCalls(message2);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalledTimes(2));
    });

    // 3rd call - Should trigger circuit breaker
    const message3 = { ...message, id: 'msg-3' };
    await act(async () => {
      result.current.processToolCalls(message3);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalledTimes(3));
    });

    // Verify circuit breaker was called on the 3rd call
    await waitFor(() => {
      const lastCall = mockExecuteToolCall.mock.calls[mockExecuteToolCall.mock.calls.length - 1];
      expect(lastCall[0].function.name).toBe('builtin_ui__circuit_break');
      expect(lastCall[0].function.arguments).toContain('test_tool');
    });
  });

  it('should reset count when a different tool is called', async () => {
    const { result } = renderHook(() => useToolProcessor({ submit: mockSubmit }));

    const toolCall1 = {
      id: 'call-1',
      type: 'function' as const,
      function: { name: 'test_tool', arguments: '{"arg":"value"}' },
    };

    const message1: Message = {
      id: 'msg-1',
      role: 'assistant',
      content: [],
      tool_calls: [toolCall1],
      sessionId: 'session-1',
      threadId: 'thread-1',
    };

    // 1st call
    await act(async () => {
      result.current.processToolCalls(message1);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalled());
    });

    // 2nd call
    const message2 = { ...message1, id: 'msg-2' };
    await act(async () => {
      result.current.processToolCalls(message2);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalledTimes(2));
    });

    // Different tool call
    const toolCallDiff = {
      id: 'call-diff',
      type: 'function' as const,
      function: { name: 'other_tool', arguments: '{}' },
    };
    const messageDiff: Message = {
      id: 'msg-diff',
      role: 'assistant',
      content: [],
      tool_calls: [toolCallDiff],
      sessionId: 'session-1',
      threadId: 'thread-1',
    };

    await act(async () => {
      result.current.processToolCalls(messageDiff);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalledTimes(3));
    });

    // Verify the different tool was called (not circuit breaker)
    await waitFor(() => {
      const lastCall = mockExecuteToolCall.mock.calls[2];
      expect(lastCall[0].function.name).toBe('other_tool');
    });

    // 3rd call of original tool (should be count 1 now, not 3, so no circuit breaker)
    const message3 = { ...message1, id: 'msg-3' };
    await act(async () => {
      result.current.processToolCalls(message3);
      await vi.waitFor(() => expect(mockExecuteToolCall).toHaveBeenCalledTimes(4));
    });

    // Verify the original tool was called again (not circuit breaker)
    await waitFor(() => {
      const lastCall = mockExecuteToolCall.mock.calls[3];
      expect(lastCall[0].function.name).toBe('test_tool');
    });
  });
});

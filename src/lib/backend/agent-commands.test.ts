import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  handleLLMResponse,
  handleLLMError,
  handleUserToolCall,
  getAgentAvailableTools,
  agentCallBuiltinTool,
} from './agent-commands';
import { safeInvoke } from './core';
import type { RustMessage } from '../../models/chat';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@paralleldrive/cuid2', () => ({
  createId: vi.fn(() => 'mock-id'),
}));

describe('backend/agent-commands', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should handleLLMError', async () => {
    await handleLLMError('session-1', 'test error');
    expect(safeInvoke).toHaveBeenCalledWith('agent_handle_llm_error', {
      sessionId: 'session-1',
      error: 'test error',
    });
  });

  it('should handleLLMResponse', async () => {
    const mockMessage = {
      id: 'msg-1',
      sessionId: 'session-1',
      role: 'assistant',
      content: [],
      createdAt: 0,
      updatedAt: 0,
    } satisfies RustMessage;
    await handleLLMResponse('session-1', mockMessage);
    expect(safeInvoke).toHaveBeenCalledWith('agent_handle_llm_response', {
      sessionId: 'session-1',
      assistantMessage: mockMessage,
    });
  });

  it('should handleUserToolCall', async () => {
    // We mock Date.now to be stable for the test
    const mockNow = 1234567890;
    vi.spyOn(Date, 'now').mockReturnValue(mockNow);

    await handleUserToolCall('session-1', 'my_tool', { foo: 'bar' });

    expect(safeInvoke).toHaveBeenCalledWith('agent_handle_llm_response', {
      sessionId: 'session-1',
      assistantMessage: {
        id: 'mock-id',
        sessionId: 'session-1',
        role: 'assistant',
        content: [],
        toolCalls: [
          {
            id: 'mock-id',
            type: 'function',
            function: {
              name: 'my_tool',
              arguments: JSON.stringify({ foo: 'bar' }),
            },
          },
        ],
        createdAt: mockNow,
        updatedAt: mockNow,
      },
    });
  });

  it('should getAgentAvailableTools', async () => {
    const mockTools = [{ name: 'tool1' }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTools);

    const res = await getAgentAvailableTools('session-1');
    expect(safeInvoke).toHaveBeenCalledWith('agent_get_available_tools', { sessionId: 'session-1' });
    expect(res).toEqual(mockTools);
  });

  it('should agentCallBuiltinTool', async () => {
    const mockResult = { content: 'test' };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResult);

    const res = await agentCallBuiltinTool('session-1', 'tool1', { arg: 1 });
    expect(safeInvoke).toHaveBeenCalledWith('agent_call_builtin_tool', {
      sessionId: 'session-1',
      toolName: 'tool1',
      args: { arg: 1 },
    });
    expect(res).toEqual(mockResult);
  });
});

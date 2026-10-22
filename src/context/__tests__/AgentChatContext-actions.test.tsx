import {
  mockSetError,
  setupAgentChatMocks,
  TestWrapper,
} from './agent-chat-test-utils';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  useAgentChatActions,
  useAgentChat,
} from '../AgentChatContext';
import { safeInvoke } from '@/lib/backend/core';
import { useAgentSessionState, useAgentSessionActions } from '../AgentSessionContext';
import type { Message } from '@/models/chat';
import { getMessagesPageForSession } from '@/lib/backend/messages';

describe('AgentChatContext', () => {
  beforeEach(() => {
    setupAgentChatMocks();
  });

  describe('Cancel Action', () => {
    it('should cancel workflow', async () => {
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.cancel();
      });

      expect(safeInvoke).toHaveBeenCalledWith('agent_cancel_workflow', {
        sessionId: 'test-session',
      });
      expect(result.current.isSessionLoading).toBe(false);
      expect(result.current.workflowStatus).toBe('idle');
    });

    it('should handle cancel errors', async () => {
      (safeInvoke as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Cancel failed'),
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.cancel();
      });

      expect(mockSetError).toHaveBeenCalledWith('Cancel failed');
    });
  });

  describe('Retry Action', () => {
    it('should retry last user message', async () => {
      const mockResumeSession = vi.fn().mockResolvedValue(undefined);

      const messagesWithError: Message[] = [
        {
          id: 'msg1',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          createdAt: new Date(),
        },
        {
          id: 'msg2',
          sessionId: 'test-session',
          threadId: 'test-session',
          role: 'assistant',
          content: [{ type: 'text', text: 'Error response' }],
          createdAt: new Date(),
          error: {
            displayMessage: 'Failed',
            type: 'AI_SERVICE_ERROR',
            recoverable: true,
          },
        },
      ];

      // Update AgentSessionContext mock to return messagesWithError
      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: { id: 'test-session', name: 'Test Session' },
        messages: messagesWithError,
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
      });

      // Update AgentSessionActions mock with resumeSession
      (useAgentSessionActions as ReturnType<typeof vi.fn>).mockReturnValue({
        setError: mockSetError,
        resumeSession: mockResumeSession,
        regenerateLastTurn: vi.fn().mockResolvedValue(undefined),
      });

      // Mock backend messages
      (getMessagesPageForSession as ReturnType<typeof vi.fn>).mockResolvedValue({
        items: messagesWithError,
        total: 2,
        page: 1,
        pageSize: 1000,
        totalPages: 1,
      });

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      // Wait for messages to load
      await waitFor(() => {
        expect(result.current.messages).toEqual(messagesWithError);
      });

      await act(async () => {
        await result.current.retryMessage();
      });

      // Should call resumeSession
      expect(mockResumeSession).toHaveBeenCalled();
    });

    it('should handle retry with no user message', async () => {
      // Mock backend messages
      // Mock backend messages (not used directly but kept for consistency)
      (getMessagesPageForSession as ReturnType<typeof vi.fn>).mockResolvedValue({
        items: [],
        total: 0,
        page: 1,
        pageSize: 1000,
        totalPages: 0,
      });

      // Mock session state with no messages
      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: { id: 'test-session', name: 'Test Session' },
        messages: [],
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
      });

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.retryMessage();
      });

      // Should call list_settings (from SettingsProvider initialization)
      // and agent_get_service_contexts (from useEffect initialization)
      // retryMessage should not trigger any additional calls
      expect(safeInvoke).toHaveBeenCalledWith('agent_get_service_contexts', {
        sessionId: 'test-session',
      });
    });

    it('should invoke agent_append_tool_messages when appendToolMessages is called', async () => {
      (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue({
        success: true,
      });

      const { result } = renderHook(() => useAgentChatActions(), {
        wrapper: TestWrapper,
      });

      const toolCallMsg: Message = {
        id: 'tc-1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'assistant',
        content: [],
        source: 'ui',
        createdAt: new Date(),
      };

      const toolResultMsg: Message = {
        id: 'tr-1',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'tool',
        content: [{ type: 'text', text: 'Success' }],
        source: 'ui',
        createdAt: new Date(),
      };

      await act(async () => {
        await result.current.appendToolMessages([toolCallMsg, toolResultMsg]);
      });

      expect(safeInvoke).toHaveBeenCalledWith('agent_append_tool_messages', {
        request: expect.objectContaining({
          sessionId: 'test-session',
          messages: expect.arrayContaining([
            expect.objectContaining({ id: 'tc-1' }),
            expect.objectContaining({ id: 'tr-1' }),
          ]),
        }),
      });
    });
  });
});

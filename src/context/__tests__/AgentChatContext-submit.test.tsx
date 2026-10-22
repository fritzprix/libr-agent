import {
  createDeferred,
  mockMessages,
  mockSetError,
  mockUnlisten,
  setupAgentChatMocks,
  TestWrapper,
} from './agent-chat-test-utils';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  useAgentChat,
} from '../AgentChatContext';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import { useAgentSessionState, useAgentSessionActions } from '../AgentSessionContext';
import type { Message } from '@/models/chat';

describe('AgentChatContext', () => {
  beforeEach(() => {
    setupAgentChatMocks();
  });

  describe('Submit Action', () => {
    it('should submit message to backend', async () => {
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      const createdAt = new Date();
      const newMessage: Message = {
        id: 'msg2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'New message' }],
        createdAt,
      };

      await act(async () => {
        await result.current.submit(newMessage);
      });

      expect(safeInvoke).toHaveBeenCalledWith('agent_inject_messages', {
        request: expect.objectContaining({
          sessionId: 'test-session',
          messages: [
            expect.objectContaining({
              ...newMessage,
              createdAt: createdAt.getTime(),
              updatedAt: createdAt.getTime(),
            }),
          ],
        }),
      });
    });

    it('should not optimistically append submitted messages to the chat list', async () => {
      const deferred = createDeferred<{ success: boolean }>();
      (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation(
        (command: string) => {
          if (command === 'agent_inject_messages') {
            return deferred.promise;
          }
          return Promise.resolve({ success: true, data: [] });
        },
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      const newMessage: Message = {
        id: 'msg-pending',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Pending message' }],
        createdAt: new Date(),
      };

      let submitPromise: Promise<void> | undefined;

      await act(async () => {
        submitPromise = result.current.submit(newMessage);
      });

      await waitFor(() => {
        expect(safeInvoke).toHaveBeenCalledWith(
          'agent_inject_messages',
          expect.anything(),
        );
      });

      expect(result.current.messages).toEqual(mockMessages);
      expect(result.current.pendingQueue).toEqual([]);

      deferred.resolve({ success: true });
      await act(async () => {
        await submitPromise;
      });
    });

    it('should inject through the backend while busy without frontend resume logic', async () => {
      const deferred = createDeferred<{ success: boolean }>();
      (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation(
        (command: string) => {
          if (command === 'agent_inject_messages') {
            return deferred.promise;
          }
          return Promise.resolve({ success: true, data: [] });
        },
      );

      const mockResumeSession = vi.fn().mockResolvedValue(undefined);
      (useAgentSessionActions as ReturnType<typeof vi.fn>).mockReturnValue({
        setError: mockSetError,
        addMessage: vi.fn(),
        resumeSession: mockResumeSession,
        regenerateLastTurn: vi.fn().mockResolvedValue(undefined),
      });

      const pendingMessage: Message = {
        id: 'msg-pending-idle',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Process me after idle' }],
        createdAt: new Date(),
      };

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        void result.current.submit(pendingMessage);
      });

      await waitFor(() => {
        expect(safeInvoke).toHaveBeenCalledWith('agent_inject_messages', {
          request: expect.objectContaining({
            sessionId: 'test-session',
            messages: [
              expect.objectContaining({
                id: pendingMessage.id,
              }),
            ],
          }),
        });
      });

      expect(result.current.pendingQueue).toEqual([]);
      expect(mockResumeSession).not.toHaveBeenCalled();

      deferred.resolve({ success: true });
      await act(async () => {
        await deferred.promise;
      });
    });

    it('updates pending queue from backend events and clears on session change', async () => {
      const eventHandlers: Array<
        (event: { payload: Record<string, unknown> }) => void
      > = [];
      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        (
          _event: string,
          handler: (event: { payload: Record<string, unknown> }) => void,
        ) => {
          eventHandlers.push(handler);
          return Promise.resolve(mockUnlisten);
        },
      );

      let sessionState = {
        session: { id: 'test-session', name: 'Test Session' },
        messages: mockMessages,
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle' as const,
      };

      (useAgentSessionState as ReturnType<typeof vi.fn>).mockImplementation(
        () => sessionState,
      );
      (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue({
        success: true,
        data: [],
      });

      const queuedRustMessage = {
        id: 'msg-pending-switch',
        sessionId: 'test-session',
        role: 'user' as const,
        content: [{ type: 'text', text: 'Carry me only in this session' }],
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };

      const { result, rerender } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandlers.length).toBeGreaterThan(0);
      });

      await act(async () => {
        for (const handler of eventHandlers) {
          handler({
            payload: {
              type: 'pendingQueueUpdated',
              sessionId: 'test-session',
              messages: [queuedRustMessage],
            },
          });
        }
      });

      await waitFor(() => {
        expect(result.current.pendingQueue).toHaveLength(1);
        expect(result.current.pendingQueue[0]?.id).toBe('msg-pending-switch');
      });

      sessionState = {
        ...sessionState,
        session: { id: 'next-session', name: 'Next Session' },
        messages: [],
      };
      rerender();

      await waitFor(() => {
        expect(result.current.pendingQueue).toEqual([]);
      });
    });

    it('should cancel a single pending prompt via IPC', async () => {
      (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue({
        success: true,
        data: { removed: true },
      });

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.cancelPendingPrompt('msg-queued');
      });

      expect(safeInvoke).toHaveBeenCalledWith('agent_cancel_pending_prompt', {
        sessionId: 'test-session',
        messageId: 'msg-queued',
      });
    });

    it('should reject submit while session is loading', async () => {
      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: { id: 'test-session', name: 'Test Session' },
        messages: mockMessages,
        isSessionLoading: true,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
      });

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      const newMessage: Message = {
        id: 'msg-loading',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Wait for init' }],
        createdAt: new Date(),
      };

      await act(async () => {
        await expect(result.current.submit(newMessage)).rejects.toThrow(
          'Cannot submit while session is still loading',
        );
      });

      expect(mockSetError).toHaveBeenCalledWith(
        'Cannot submit while session is still loading',
      );
      expect(safeInvoke).not.toHaveBeenCalledWith(
        'agent_inject_messages',
        expect.anything(),
      );
    });

    it('should handle submit errors', async () => {
      (safeInvoke as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Submit failed'),
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      const newMessage: Message = {
        id: 'msg2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'New message' }],
        createdAt: new Date(),
      };

      await act(async () => {
        await expect(result.current.submit(newMessage)).rejects.toThrow('Submit failed');
      });

      expect(mockSetError).toHaveBeenCalledWith('Submit failed');
      expect(result.current.messages).toEqual(mockMessages);
    });

    it('should not submit without active session', async () => {
      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: null,
        messages: [],
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
      });
      console.error = vi.fn(); // Suppress error logs
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      const newMessage: Message = {
        id: 'msg2',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'New message' }],
        createdAt: new Date(),
      };

      await act(async () => {
        await result.current.submit(newMessage);
      });

      // SettingsProvider calls list_settings on initialization
      // But submit should not call any agent-related commands
      expect(safeInvoke).not.toHaveBeenCalledWith(
        expect.stringMatching(/^agent_/),
        expect.anything(),
      );
    });
  });
});

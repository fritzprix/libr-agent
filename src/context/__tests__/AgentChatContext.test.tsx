import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  AgentChatProvider,
  useAgentChatState,
  useAgentChatActions,
  useAgentChat,
} from '../AgentChatContext';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useAgentSessionState } from '../AgentSessionContext';
import type { Message } from '@/models/chat';
import {
  getMessagesPageForSession,
  deleteMessage,
} from '@/lib/backend/messages';
import { LLMServiceProvider } from '../LLMServiceContext';
import { SettingsProvider } from '../SettingsContext';
import type { ReactNode } from 'react';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock AgentSessionContext
vi.mock('../AgentSessionContext', () => ({
  useAgentSessionState: vi.fn(),
}));

// Mock backend messages API
vi.mock('@/lib/backend/messages', () => ({
  getMessagesPageForSession: vi.fn(),
  deleteMessage: vi.fn(),
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

// Test wrapper with required providers
function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <SettingsProvider>
      <LLMServiceProvider>
        <AgentChatProvider>{children}</AgentChatProvider>
      </LLMServiceProvider>
    </SettingsProvider>
  );
}

describe('AgentChatContext', () => {
  const mockUnlisten = vi.fn();

  const mockMessages: Message[] = [
    {
      id: 'msg1',
      sessionId: 'test-session',
      threadId: 'test-session',
      role: 'user',
      content: [{ type: 'text', text: 'Hello' }],
      createdAt: new Date(),
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();

    // Setup listen mock
    (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);

    // Setup AgentSessionContext mock
    (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
      currentSession: { id: 'test-session', name: 'Test Session' },
      messages: mockMessages,
      isLoading: false,
      error: null,
    });

    // Setup backend messages mock
    (getMessagesPageForSession as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: mockMessages,
      total: 1,
      page: 1,
      pageSize: 1000,
      totalPages: 1,
    });

    // Setup invoke mock
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
      success: true,
    });
  });

  describe('Provider Setup', () => {
    it('should provide state context', async () => {
      const { result } = renderHook(() => useAgentChatState(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(result.current.messages).toEqual(mockMessages);
      });

      expect(result.current).toBeDefined();
      expect(result.current.isLoading).toBe(false);
      expect(result.current.error).toBeNull();
      expect(result.current.workflowStatus).toBe('idle');
    });

    it('should provide actions context', () => {
      const { result } = renderHook(() => useAgentChatActions(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(typeof result.current.submit).toBe('function');
      expect(typeof result.current.cancel).toBe('function');
      expect(typeof result.current.retryMessage).toBe('function');
    });

    it('should provide combined hook', () => {
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      expect(result.current).toBeDefined();
      expect(result.current.isLoading).toBe(false);
      expect(typeof result.current.submit).toBe('function');
    });

    it('should throw error when state hook used outside provider', () => {
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useAgentChatState());
      }).toThrow('useAgentChatState must be used within AgentChatProvider');

      console.error = originalError;
    });

    it('should throw error when actions hook used outside provider', () => {
      const originalError = console.error;
      console.error = vi.fn();

      expect(() => {
        renderHook(() => useAgentChatActions());
      }).toThrow('useAgentChatActions must be used within AgentChatProvider');

      console.error = originalError;
    });
  });

  describe('Event Listeners', () => {
    it('should register event listener on mount', async () => {
      renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalledWith('agent:event', expect.any(Function));
      });
    });

    it('should cleanup on unmount', async () => {
      const { unmount } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(listen).toHaveBeenCalled();
      });

      unmount();

      await waitFor(() => {
        expect(mockUnlisten).toHaveBeenCalled();
      });
    });

    it('should update status on agent event', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'agent:event') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      // Trigger busy status
      act(() => {
        eventHandler?.({
          payload: {
            type: 'statusChanged',
            session_id: 'test-session',
            status: 'Busy',
          },
        });
      });

      await waitFor(() => {
        expect(result.current.workflowStatus).toBe('busy');
        expect(result.current.isLoading).toBe(true);
      });

      // Trigger idle status
      act(() => {
        eventHandler?.({
          payload: {
            type: 'statusChanged',
            session_id: 'test-session',
            status: 'Idle',
          },
        });
      });

      await waitFor(() => {
        expect(result.current.workflowStatus).toBe('idle');
        expect(result.current.isLoading).toBe(false);
      });
    });

    it('should ignore events for different sessions', async () => {
      let eventHandler: ((event: unknown) => void) | undefined;

      (listen as ReturnType<typeof vi.fn>).mockImplementation(
        async (eventName, handler) => {
          if (eventName === 'agent:event') {
            eventHandler = handler as (event: unknown) => void;
          }
          return mockUnlisten;
        },
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await waitFor(() => {
        expect(eventHandler).toBeDefined();
      });

      const initialStatus = result.current.workflowStatus;

      // Trigger event for different session
      act(() => {
        eventHandler?.({
          payload: {
            type: 'statusChanged',
            session_id: 'other-session',
            status: 'Busy',
          },
        });
      });

      // Status should remain unchanged
      expect(result.current.workflowStatus).toBe(initialStatus);
    });
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

      // Expect Date to be converted to Unix timestamp
      expect(invoke).toHaveBeenCalledWith('agent_send_message', {
        request: {
          sessionId: 'test-session',
          message: {
            ...newMessage,
            createdAt: createdAt.getTime(),
            updatedAt: createdAt.getTime(),
          },
        },
      });
    });

    it('should handle submit errors', async () => {
      (invoke as ReturnType<typeof vi.fn>).mockRejectedValue(
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
        await result.current.submit(newMessage);
      });

      expect(result.current.error).toBe('Submit failed');
    });

    it('should not submit without active session', async () => {
      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        currentSession: null,
      });

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

      expect(invoke).not.toHaveBeenCalled();
    });
  });

  describe('Cancel Action', () => {
    it('should cancel workflow', async () => {
      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.cancel();
      });

      expect(invoke).toHaveBeenCalledWith('agent_terminate_workflow', {
        sessionId: 'test-session',
      });
      expect(result.current.isLoading).toBe(false);
      expect(result.current.workflowStatus).toBe('idle');
    });

    it('should handle cancel errors', async () => {
      (invoke as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error('Cancel failed'),
      );

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.cancel();
      });

      expect(result.current.error).toBe('Cancel failed');
    });
  });

  describe('Retry Action', () => {
    it('should retry last user message', async () => {
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
        currentSession: { id: 'test-session', name: 'Test Session' },
        messages: messagesWithError,
        isLoading: false,
        error: null,
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

      // Should delete the error message
      expect(deleteMessage).toHaveBeenCalledWith('msg2');

      // Should re-submit the user message
      expect(invoke).toHaveBeenCalledWith('agent_send_message', {
        request: {
          sessionId: 'test-session',
          message: expect.objectContaining({
            id: 'msg1',
            role: 'user',
          }),
        },
      });
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
        currentSession: { id: 'test-session', name: 'Test Session' },
        messages: [],
        isLoading: false,
        error: null,
      });

      const { result } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      await act(async () => {
        await result.current.retryMessage();
      });

      // Should not invoke anything
      expect(invoke).not.toHaveBeenCalled();
    });
  });
});

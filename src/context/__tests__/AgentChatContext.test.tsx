import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  AgentChatProvider,
  useAgentChatState,
  useAgentChatActions,
  useAgentChat,
} from '../AgentChatContext';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import { useAgentSessionState, useAgentSessionActions } from '../AgentSessionContext';
import type { Message } from '@/models/chat';
import { getMessagesPageForSession } from '@/lib/backend/messages';
import { LLMServiceProvider } from '../LLMServiceContext';
import { SettingsProvider } from '../SettingsContext';
import type { ReactNode } from 'react';

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

// Mock AgentSessionContext
vi.mock('../AgentSessionContext', () => ({
  useAgentSessionState: vi.fn(),
  useAgentSessionActions: vi.fn(),
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
  const mockSetError = vi.fn(); // Added mock

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
      session: { id: 'test-session', name: 'Test Session' },
      messages: mockMessages,
      isSessionLoading: false,
      error: null,
      llmError: null,
      workflowStatus: 'idle',
    });

    // Setup AgentSessionActions mock
    (useAgentSessionActions as ReturnType<typeof vi.fn>).mockReturnValue({
      setError: mockSetError,
      addMessage: vi.fn(),
      resumeSession: vi.fn().mockResolvedValue(undefined),
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
    (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue({
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
      expect(result.current.isSessionLoading).toBe(false);
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
      expect(result.current.isSessionLoading).toBe(false);
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
      expect(safeInvoke).toHaveBeenCalledWith('agent_send_message', {
        request: expect.objectContaining({
          sessionId: 'test-session',
          message: expect.objectContaining({
            ...newMessage,
            createdAt: createdAt.getTime(),
            updatedAt: createdAt.getTime(),
          }),
        }),
      });
    });

    it('should render pending message before backend send resolves', async () => {
      const deferred = createDeferred<{ success: boolean }>();
      (safeInvoke as ReturnType<typeof vi.fn>).mockReturnValue(deferred.promise);

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
        expect(result.current.messages).toEqual([...mockMessages, newMessage]);
      });

      deferred.resolve({ success: true });
      await act(async () => {
        await submitPromise;
      });
    });

    it('should dedupe pending and persisted messages with the same id', async () => {
      const duplicatedMessage: Message = {
        id: 'msg-duplicate',
        sessionId: 'test-session',
        threadId: 'test-session',
        role: 'user',
        content: [{ type: 'text', text: 'Duplicate message' }],
        createdAt: new Date(),
      };

      const deferred = createDeferred<{ success: boolean }>();
      (safeInvoke as ReturnType<typeof vi.fn>).mockReturnValue(deferred.promise);

      const { result, rerender } = renderHook(() => useAgentChat(), {
        wrapper: TestWrapper,
      });

      let submitPromise: Promise<void> | undefined;

      await act(async () => {
        submitPromise = result.current.submit(duplicatedMessage);
      });

      await waitFor(() => {
        expect(
          result.current.messages.filter((message) => message.id === duplicatedMessage.id),
        ).toHaveLength(1);
      });

      (useAgentSessionState as ReturnType<typeof vi.fn>).mockReturnValue({
        session: { id: 'test-session', name: 'Test Session' },
        messages: [...mockMessages, duplicatedMessage],
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
      });

      rerender();

      await waitFor(() => {
        expect(
          result.current.messages.filter((message) => message.id === duplicatedMessage.id),
        ).toHaveLength(1);
      });

      deferred.resolve({ success: true });
      await act(async () => {
        await submitPromise;
      });
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
  });
});

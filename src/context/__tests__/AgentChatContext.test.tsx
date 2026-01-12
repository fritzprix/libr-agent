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
import { useAgentSessionState, useAgentSessionActions } from '../AgentSessionContext';
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

import { SystemPromptProvider } from '../SystemPromptContext';

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
      <SystemPromptProvider>
        <LLMServiceProvider>
          <AgentChatProvider>{children}</AgentChatProvider>
        </LLMServiceProvider>
      </SystemPromptProvider>
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

      expect(mockSetError).toHaveBeenCalledWith('Submit failed');
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
      expect(invoke).toHaveBeenCalledWith('list_settings');
      expect(invoke).not.toHaveBeenCalledWith(
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

      expect(invoke).toHaveBeenCalledWith('agent_terminate_workflow', {
        sessionId: 'test-session',
      });
      expect(result.current.isSessionLoading).toBe(false);
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

      expect(mockSetError).toHaveBeenCalledWith('Cancel failed');
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
        session: { id: 'test-session', name: 'Test Session' },
        messages: messagesWithError,
        isSessionLoading: false,
        error: null,
        llmError: null,
        workflowStatus: 'idle',
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
      expect(invoke).toHaveBeenCalledTimes(2);
      expect(invoke).toHaveBeenCalledWith('list_settings');
      expect(invoke).toHaveBeenCalledWith('agent_get_service_contexts', {
        sessionId: 'test-session',
      });
    });
  });
});

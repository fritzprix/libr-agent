import { vi } from 'vitest';
import type { ReactNode } from 'react';
import { AgentChatProvider } from '../AgentChatContext';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from '../AgentSessionContext';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message } from '@/models/chat';
import { getMessagesPageForSession } from '@/lib/backend/messages';
import { LLMServiceProvider } from '../LLMServiceContext';
import { TestSettingsProvider } from '@/test/fixtures/TestSettingsProvider';

export function createDeferred<T>() {
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

vi.mock('@/lib/ai-service/factory', () => ({
  AIServiceFactory: {
    getService: vi.fn(),
  },
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
export function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <TestSettingsProvider>
      <LLMServiceProvider>
        <AgentChatProvider>{children}</AgentChatProvider>
      </LLMServiceProvider>
    </TestSettingsProvider>
  );
}

export const mockUnlisten = vi.fn();
export const mockSetError = vi.fn();
export const mockStreamChat = vi.fn();
export const mockListModels = vi.fn();
export const mockDispose = vi.fn();

export const mockMessages: Message[] = [
  {
    id: 'msg1',
    sessionId: 'test-session',
    threadId: 'test-session',
    role: 'user',
    content: [{ type: 'text', text: 'Hello' }],
    createdAt: new Date(),
  },
];

export function setupAgentChatMocks() {
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
    regenerateLastTurn: vi.fn().mockResolvedValue(undefined),
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

  (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
    streamChat: mockStreamChat,
    listModels: mockListModels,
    dispose: mockDispose,
    sanitizeMessages: vi.fn((messages: Message[]) => messages),
  });

  mockListModels.mockResolvedValue([
    {
      name: 'test-model',
      contextWindow: 4096,
      supportReasoning: false,
      supportTools: true,
      supportStreaming: true,
      cost: { input: 0.001, output: 0.002 },
      description: 'Test model for unit tests',
    },
  ]);
}

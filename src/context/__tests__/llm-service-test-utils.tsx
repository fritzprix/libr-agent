import { beforeEach, vi } from 'vitest';
import type { ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { AIServiceFactory } from '@/lib/ai-service/factory';
import type { Message } from '@/models/chat';
import {
  LLMServiceProvider,
  useLLMService,
  useStreamingMessages,
} from '../LLMServiceContext';
import { __resetLLMListenerStartupLogStateForTests } from '../llm/useLLMListener';
import { TestSettingsProvider } from '@/test/fixtures/TestSettingsProvider';

const { loggerInfo, loggerDebug, loggerWarn, loggerError } = vi.hoisted(() => ({
  loggerInfo: vi.fn(),
  loggerDebug: vi.fn(),
  loggerWarn: vi.fn(),
  loggerError: vi.fn(),
}));

export { loggerInfo, loggerDebug, loggerWarn, loggerError };

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock agent commands
vi.mock('@/lib/backend/agent-commands', () => ({
  handleLLMError: vi.fn(),
  handleLLMResponse: vi.fn(),
  reportLLMStreamingIssue: vi.fn(),
  getAgentCompactContext: vi.fn(),
}));

// Mock AIServiceFactory
vi.mock('@/lib/ai-service/factory', () => ({
  AIServiceFactory: {
    getService: vi.fn(),
  },
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: loggerInfo,
    debug: loggerDebug,
    warn: loggerWarn,
    error: loggerError,
  }),
}));

// Mock retry-utils
vi.mock('@/lib/retry-utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/retry-utils')>();
  return { ...actual, sleep: vi.fn().mockResolvedValue(undefined) };
});

// Test wrapper with required providers
export function TestWrapper({ children }: { children: ReactNode }) {
  return (
    <TestSettingsProvider>
      <LLMServiceProvider>{children}</LLMServiceProvider>
    </TestSettingsProvider>
  );
}

export function useLLMServiceHarness() {
  return {
    ...useLLMService(),
    streamingMessages: useStreamingMessages(),
  };
}

export function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

export function setupLLMCompletionMocks() {
  const mockUnlisten = vi.fn();
  const mockStreamChat = vi.fn();
  const mockListModels = vi.fn();
  const mockCancel = vi.fn();
  const mockDispose = vi.fn();
  const mockSetDefaultConfig = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    __resetLLMListenerStartupLogStateForTests();

    (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);

    (AIServiceFactory.getService as ReturnType<typeof vi.fn>).mockReturnValue({
      streamChat: mockStreamChat,
      listModels: mockListModels,
      cancel: mockCancel,
      dispose: mockDispose,
      setDefaultConfig: mockSetDefaultConfig,
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
  });

  return {
    mockUnlisten,
    mockStreamChat,
    mockListModels,
    mockCancel,
    mockDispose,
    mockSetDefaultConfig,
  };
}

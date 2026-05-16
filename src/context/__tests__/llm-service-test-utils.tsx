import { vi } from 'vitest';
import type { ReactNode } from 'react';
import {
  LLMServiceProvider,
  useLLMService,
  useStreamingMessages,
} from '../LLMServiceContext';
import { SettingsProvider } from '../SettingsContext';

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
  invoke: vi.fn(),
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
    <SettingsProvider>
      <LLMServiceProvider>{children}</LLMServiceProvider>
    </SettingsProvider>
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

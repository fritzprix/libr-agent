import { vi } from 'vitest';
import React from 'react';
import {
  AgentSessionProvider,
  useAgentSessionState,
} from '../AgentSessionContext';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import * as agentCommandsBackend from '@/lib/backend/agent-commands';
import type { Message } from '@/models/chat';

export const mockMarkSessionViewed = vi.fn();
export const mockClearPendingApproval = vi.fn();
export const mockRenameSession = vi.fn();
export const mockRefreshCompactedRange = vi.fn();
export const listenMock = listen as ReturnType<typeof vi.fn>;
export const safeInvokeMock = safeInvoke as ReturnType<typeof vi.fn>;
export const openAgentSessionMock =
  agentCommandsBackend.openAgentSession as ReturnType<typeof vi.fn>;

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
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

vi.mock('@/lib/backend/agent-commands', () => ({
  openAgentSession: vi.fn(),
}));

vi.mock('../AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    markSessionViewed: mockMarkSessionViewed,
    clearPendingApproval: mockClearPendingApproval,
    renameSession: mockRenameSession,
  }),
}));

vi.mock('../LLMServiceContext', () => ({
  useLLMService: () => ({
    refreshCompactedRange: mockRefreshCompactedRange,
  }),
}));

// Mock ModelProvider
vi.mock('../ModelProvider', () => ({
  useModelOptions: () => ({
    modelId: 'test-model',
    provider: 'test-provider',
  }),
}));

export const TEST_SESSION_ID = 'session-1';

export type OpenAgentSessionResponse = Awaited<
  ReturnType<typeof agentCommandsBackend.openAgentSession>
>;
export type RuntimeState = OpenAgentSessionResponse['runtimeState'];
export type AgentSessionStateSnapshot = ReturnType<typeof useAgentSessionState>;

export const READY_RUNTIME_STATE = {
  sequence: 1,
  phase: 'ready' as const,
  proxy: {
    exists: true,
    mode: 'builtin_only' as const,
    ready: true,
  },
  initialization: {
    currentStep: 'Session initialization complete',
    result: 'success' as const,
  },
  servers: [],
};

export function createReadyRuntimeState(
  overrides: Partial<RuntimeState> = {},
): RuntimeState {
  return {
    ...READY_RUNTIME_STATE,
    ...overrides,
    proxy: {
      ...READY_RUNTIME_STATE.proxy,
      ...overrides.proxy,
    },
    initialization: {
      ...READY_RUNTIME_STATE.initialization,
      ...overrides.initialization,
    },
    servers: overrides.servers ?? [],
  };
}

export function createOpenSessionResponse(
  sessionId: string,
  overrides: {
    session?: Partial<OpenAgentSessionResponse['session']>;
    messages?: OpenAgentSessionResponse['messages']['items'];
    hasMoreBefore?: boolean;
    oldestCursor?: OpenAgentSessionResponse['messages']['oldestCursor'];
    pendingApprovals?: OpenAgentSessionResponse['pendingApprovals'];
    runtimeState?: RuntimeState;
  } = {},
): OpenAgentSessionResponse {
  const timestamp = Date.now();

  return {
    session: {
      id: sessionId,
      name: `Session ${sessionId}`,
      status: 'idle',
      model: 'test-model',
      provider: 'test-provider',
      createdAt: timestamp,
      updatedAt: timestamp,
      yoloMode: false,
      ...overrides.session,
    },
    messages: {
      items: overrides.messages ?? [],
      hasMoreBefore: overrides.hasMoreBefore ?? false,
      oldestCursor: overrides.oldestCursor ?? null,
    },
    pendingApprovals: overrides.pendingApprovals ?? [],
    runtimeState: overrides.runtimeState ?? READY_RUNTIME_STATE,
  };
}

export function SessionWrapper({
  children,
  sessionId,
}: React.PropsWithChildren<{ sessionId: string }>) {
  return (
    <AgentSessionProvider sessionId={sessionId}>
      {children}
    </AgentSessionProvider>
  );
}

export function createDefaultWrapper(sessionId = TEST_SESSION_ID) {
  return function DefaultWrapper({ children }: React.PropsWithChildren) {
    return <SessionWrapper sessionId={sessionId}>{children}</SessionWrapper>;
  };
}

export function buildTestMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: TEST_SESSION_ID,
    threadId: TEST_SESSION_ID,
    role: 'user',
    content: [{ type: 'text', text: 'Hello' }],
    createdAt: new Date(),
    ...overrides,
    // NOTE: `as Message` is required because TypeScript cannot verify that
    // Partial<Message> spread satisfies all required fields at compile time.
    // This is safe because we always provide default values for all fields.
  } as Message;
}

export function AgentSessionStateObserver({
  onRender,
}: {
  onRender: (state: AgentSessionStateSnapshot) => void;
}) {
  const state = useAgentSessionState();
  onRender(state);
  return null;
}

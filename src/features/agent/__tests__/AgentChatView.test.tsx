import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AgentSessionStateContextValue } from '@/context/agent-session/types';
import type { AgentSession } from '@/models/agent';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import AgentChatView from '../AgentChatView';

const mocks = vi.hoisted(() => ({
  agentSessionState: undefined as AgentSessionStateContextValue | undefined,
}));

function createBaseRuntimeState(): SessionRuntimeState {
  return {
    sequence: 0,
    phase: 'not_started',
    proxy: {
      exists: false,
      mode: 'none',
      ready: false,
    },
    initialization: {
      result: 'pending',
    },
    servers: [],
  };
}

function createBaseSessionState(): AgentSessionStateContextValue {
  return {
    session: null,
    messages: [],
    isSessionLoading: false,
    isLoadingOlderMessages: false,
    hasOlderMessages: false,
    error: null,
    llmError: null,
    workflowStatus: 'idle',
    workflowPhase: 'idle',
    runtimeState: createBaseRuntimeState(),
    initializationStep: null,
    pendingApprovals: [],
    executionMode: 'normal',
    yoloModeEnabled: false,
    unsafeModeEnabled: false,
  };
}

function createSessionState(
  overrides: Partial<AgentSessionStateContextValue> = {},
): AgentSessionStateContextValue {
  return {
    ...createBaseSessionState(),
    ...overrides,
  };
}

function createMockSession(): AgentSession {
  return {
    id: 'session-1',
    name: 'Session One',
    status: 'idle',
    model: 'gpt-5.4',
    provider: 'openai',
    assistant: {
      id: 'assistant-1',
      name: 'Assistant One',
      systemPrompt: 'You are helpful.',
      allowedBuiltInServiceAliases: [],
      deletionProtected: false,
      createdAt: new Date(),
      updatedAt: new Date(),
    },
    createdAt: new Date(),
    updatedAt: new Date(),
    yoloMode: false,
  };
}

vi.mock('react-router-dom', () => ({
  useParams: () => ({ sessionId: 'session-2' }),
  useSearchParams: () => [new URLSearchParams(), vi.fn()],
}));

vi.mock('@/context/AgentSessionContext', () => ({
  AgentSessionProvider: (props: { children: ReactNode; sessionId: string }) => (
    <>{props.children}</>
  ),
  useOptionalAgentSessionState: () =>
    mocks.agentSessionState ?? createBaseSessionState(),
  useAgentSessionState: () => mocks.agentSessionState ?? createBaseSessionState(),
}));

vi.mock('@/context/AgentChatContext', () => ({
  AgentChatProvider: ({ children }: { children: ReactNode }) => (
    <div data-testid="chat-provider">{children}</div>
  ),
  useAgentChatActions: () => ({
    injectMessages: vi.fn(),
  }),
  useAgentChatState: () => ({
    workflowStatus: 'idle' as const,
  }),
}));

vi.mock('@/context/AgentWorkspaceContext', () => ({
  AgentWorkspaceProvider: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
  useAgentWorkspace: () => ({
    showWorkspacePanel: false,
  }),
}));

vi.mock('@/context/AgentPlanningContext', () => ({
  AgentPlanningProvider: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
  useAgentPlanning: () => ({
    showPlanningPanel: false,
  }),
}));

vi.mock('../hooks/useAgentResourceAttachment', () => ({
  AgentResourceAttachmentProvider: (props: {
    children: ReactNode;
    sessionId: string;
  }) => <>{props.children}</>,
}));

vi.mock('../components/AgentChatHeader', () => ({
  AgentChatHeader: () => <div>mock-header</div>,
}));

vi.mock('../components/AgentChatStatusBar', () => ({
  AgentChatStatusBar: () => <div>mock-status-bar</div>,
}));

vi.mock('../components/AgentChatMessages', () => ({
  AgentChatMessages: () => <div>mock-messages</div>,
}));

vi.mock('../components/AgentChatInput', () => ({
  AgentChatInput: () => <div>mock-input</div>,
}));

vi.mock('../components/AgentChatAttachedFiles', () => ({
  AgentChatAttachedFiles: () => <div>mock-attached-files</div>,
}));

vi.mock('../components/AgentWorkspacePanel', () => ({
  AgentWorkspacePanel: () => <div>mock-workspace-panel</div>,
}));

vi.mock('../components/AgentPlanningPanel', () => ({
  AgentPlanningPanel: () => <div>mock-planning-panel</div>,
}));

vi.mock('../components/AgentPlanningUpdates', () => ({
  AgentPlanningUpdates: () => <div>mock-planning-updates</div>,
}));

vi.mock('@/components/ui/LoadingSpinner', () => ({
  default: ({ label }: { label?: string }) => <div>{label ?? 'spinner'}</div>,
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  agentCallBuiltinTool: vi.fn(),
}));

vi.mock('@/lib/chat-utils', () => ({
  createToolMessagePair: vi.fn(),
}));

vi.mock('@paralleldrive/cuid2', () => ({
  createId: () => 'tool-call-id',
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('AgentChatView', () => {
  beforeEach(() => {
    mocks.agentSessionState = createSessionState();
  });

  it('shows the blocking loader when there is no hydrated session yet', () => {
    mocks.agentSessionState = createSessionState({
      isSessionLoading: true,
      runtimeState: {
        ...createBaseRuntimeState(),
        phase: 'hydrating',
      },
      initializationStep: {
        step: 'Opening session',
        status: 'running' as const,
      },
    });

    render(<AgentChatView />);

    expect(screen.getAllByText('Loading session...')).not.toHaveLength(0);
    expect(screen.queryByText('mock-messages')).not.toBeInTheDocument();
  });

  it('keeps the current chat mounted while the next session is hydrating', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
      isSessionLoading: true,
      runtimeState: {
        ...createBaseRuntimeState(),
        phase: 'hydrating',
      },
      initializationStep: {
        step: 'Opening session',
        status: 'running' as const,
      },
    });

    render(<AgentChatView />);

    expect(screen.getByText('mock-messages')).toBeInTheDocument();
    expect(screen.getByText('mock-header')).toBeInTheDocument();
    expect(screen.getAllByText('Loading session...')).not.toHaveLength(0);
    expect(screen.getByTestId('chat-provider')).toBeInTheDocument();
  });
});

import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';

import type { AgentSessionStateContextValue } from '@/context/agent-session/types';
import type { AgentSession } from '@/models/agent';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import AgentChatView from '../AgentChatView';

const mocks = vi.hoisted(() => ({
  agentSessionState: undefined as AgentSessionStateContextValue | undefined,
  isMobile: false,
  showSidePanel: false,
  activeTab: 'workspace' as 'workspace' | 'planning' | 'processes',
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
    isProxyReady: true,
    isLoadingOlderMessages: false,
    hasOlderMessages: false,
    error: null,
    llmError: null,
    workflowStatus: 'idle',
    workflowPhase: 'idle',
    runtimeState: createBaseRuntimeState(),
    preflightTokenMetrics: null,
    initializationStep: null,
    pendingApprovals: [],
    pendingInteractiveShellPrompt: null,
    executionMode: 'normal',
  };
}

function createSessionState(
  overrides: Partial<AgentSessionStateContextValue> = {},
): AgentSessionStateContextValue {
  const base = createBaseSessionState();
  const merged = {
    ...base,
    ...overrides,
  };
  const isProxyReady =
    overrides.isProxyReady ?? merged.runtimeState.proxy.ready;
  return {
    ...merged,
    isProxyReady,
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
    executionMode: 'normal',
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

vi.mock('@/context/AgentPanelsContext', () => ({
  AgentPanelsProvider: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
  useAgentPanels: () => ({
    isShellOpen: () => mocks.showSidePanel,
    openShell: vi.fn(),
    closeShell: vi.fn(),
    toggleShell: vi.fn(),
    activeTab: mocks.activeTab,
    setActiveTab: vi.fn(),
    isPanelOpen: (id: 'workspace' | 'planning' | 'processes') =>
      mocks.showSidePanel && mocks.activeTab === id,
    openPanel: vi.fn(),
    closePanel: vi.fn(),
    togglePanel: vi.fn(),
    closeAllPanels: vi.fn(),
    getLastClosedAt: () => 0,
    hasPanelAttention: () => false,
    markPanelAttention: vi.fn(),
    clearPanelAttention: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => mocks.isMobile,
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

vi.mock('../components/AgentSidePanelShell', () => ({
  AgentSidePanelShell: () => <div>mock-side-panel-shell</div>,
}));

vi.mock('../components/AgentPlanningUpdates', () => ({
  AgentPlanningUpdates: () => <div>mock-planning-updates</div>,
}));

vi.mock('../components/AgentProcessAttentionUpdates', () => ({
  AgentProcessAttentionUpdates: () => null,
}));

vi.mock('@/components/ui/sheet', () => ({
  Sheet: ({
    children,
    open,
  }: {
    children: ReactNode;
    open: boolean;
    onOpenChange?: (open: boolean) => void;
  }) => <div data-testid={open ? 'sheet-open' : 'sheet-closed'}>{children}</div>,
  SheetContent: ({
    children,
    side,
  }: {
    children: ReactNode;
    side: string;
    className?: string;
  }) => <div data-testid={`sheet-content-${side}`}>{children}</div>,
  SheetHeader: ({ children }: { children: ReactNode }) => <>{children}</>,
  SheetTitle: ({ children }: { children: ReactNode }) => <>{children}</>,
  SheetDescription: ({ children }: { children: ReactNode }) => <>{children}</>,
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
    warning: vi.fn(),
    loading: vi.fn(),
    dismiss: vi.fn(),
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
    mocks.isMobile = false;
    mocks.showSidePanel = false;
    mocks.activeTab = 'workspace';
    vi.mocked(toast.loading).mockClear();
    vi.mocked(toast.success).mockClear();
    vi.mocked(toast.warning).mockClear();
    vi.mocked(toast.error).mockClear();
    vi.mocked(toast.dismiss).mockClear();
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
      isProxyReady: false,
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
    expect(screen.getByTestId('chat-provider')).toBeInTheDocument();
    expect(toast.loading).toHaveBeenCalledWith(
      'Opening session',
      expect.objectContaining({ id: 'mcp-discovery:session-1' }),
    );
  });

  it('shows discovery loading toast during proxy initialization', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
      isSessionLoading: false,
      isProxyReady: false,
      runtimeState: {
        ...createBaseRuntimeState(),
        phase: 'initializing',
        servers: [
          {
            name: 'arxiv',
            transport: 'stdio',
            status: 'connecting',
            toolCount: 0,
          },
          {
            name: 'exa',
            transport: 'http',
            status: 'discovering_tools',
            toolCount: 0,
          },
        ],
        initialization: {
          result: 'pending',
          currentStep: 'Loading MCP: arxiv, exa (0/2)',
        },
      },
      initializationStep: {
        step: 'Loading MCP: arxiv, exa (0/2)',
        status: 'running' as const,
      },
    });

    render(<AgentChatView />);

    expect(screen.getByText('mock-messages')).toBeInTheDocument();
    expect(screen.queryByTestId('mcp-server-status-list')).not.toBeInTheDocument();
    expect(toast.loading).toHaveBeenCalledWith(
      'Loading MCP: arxiv, exa (0/2)',
      expect.objectContaining({ id: 'mcp-discovery:session-1' }),
    );
  });

  it('shows partial discovery warning toast without top banner', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
      isProxyReady: true,
      runtimeState: {
        ...createBaseRuntimeState(),
        phase: 'degraded',
        proxy: {
          exists: true,
          mode: 'configured',
          ready: true,
        },
        servers: [
          {
            name: 'arxiv',
            transport: 'stdio',
            status: 'failed',
            toolCount: 0,
            error: 'stdio spawn timed out',
          },
          {
            name: 'exa',
            transport: 'http',
            status: 'ready',
            toolCount: 4,
          },
        ],
        initialization: {
          result: 'partial',
          currentStep: 'MCP partial: arxiv failed (1/2 ready)',
          error: '1 of 2 MCP servers failed',
        },
      },
    });

    render(<AgentChatView />);

    expect(screen.getByText('mock-messages')).toBeInTheDocument();
    expect(
      screen.queryByText('Some MCP servers failed or timed out'),
    ).not.toBeInTheDocument();
    expect(toast.warning).toHaveBeenCalledWith(
      'Some MCP servers failed or timed out',
      expect.objectContaining({ duration: 5000 }),
    );
  });

  it('shows success discovery toast when MCP is ready', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
      isProxyReady: true,
      runtimeState: {
        ...createBaseRuntimeState(),
        phase: 'ready',
        proxy: {
          exists: true,
          mode: 'configured',
          ready: true,
        },
        servers: [
          {
            name: 'exa',
            transport: 'http',
            status: 'ready',
            toolCount: 4,
          },
        ],
        initialization: {
          result: 'success',
          currentStep: 'MCP ready: exa',
        },
      },
    });

    render(<AgentChatView />);

    expect(screen.getByText('mock-messages')).toBeInTheDocument();
    expect(screen.queryByText('MCP servers ready')).not.toBeInTheDocument();
    expect(toast.success).toHaveBeenCalledWith(
      'MCP servers ready',
      expect.objectContaining({ duration: 2500 }),
    );
  });
  it('renders the desktop side panel shell on the right', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
    });
    mocks.showSidePanel = true;

    render(<AgentChatView />);

    expect(screen.getByText('mock-side-panel-shell')).toBeInTheDocument();
    expect(screen.queryByTestId('sheet-content-left')).not.toBeInTheDocument();
    expect(screen.queryByTestId('sheet-content-right')).not.toBeInTheDocument();
  });

  it('renders the mobile side panel shell inside a right sheet', () => {
    mocks.agentSessionState = createSessionState({
      session: createMockSession(),
    });
    mocks.isMobile = true;
    mocks.showSidePanel = true;

    render(<AgentChatView />);

    expect(screen.getByTestId('sheet-content-right')).toBeInTheDocument();
    expect(screen.queryByTestId('sheet-content-left')).not.toBeInTheDocument();
    expect(screen.getByText('mock-side-panel-shell')).toBeInTheDocument();
  });
});

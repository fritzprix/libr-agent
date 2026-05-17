import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ExecutionMode } from '@/context/agent-session/types';
import type { TokenUsage } from '@/lib/ai-service/types';
import { AgentChatStatusBar } from '../AgentChatStatusBar';

const mocks = vi.hoisted(() => ({
  safeInvoke: vi.fn(),
  updateSessionConfig: vi.fn(),
  setExecutionMode: vi.fn(),
  retryMessage: vi.fn(),
  resume: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

const viewportState = vi.hoisted(() => ({
  isMobile: false,
}));

const metricsBadgeState = vi.hoisted(() => ({
  usage: null as TokenUsage | null,
  compact: false,
  hasCompactionPressure: false,
}));

const tokenMetricsState = vi.hoisted(() => ({
  metrics: null as TokenUsage | null,
}));

type WorkflowStatus = 'idle' | 'busy' | 'paused' | 'error';

interface MockSession {
  id: string;
  model: string;
  provider: string;
  assistant: {
    name: string;
    systemPrompt: string;
    allowedBuiltInServiceAliases: string[];
  };
}

const mockSession: MockSession = {
  id: 'session-123',
  model: 'gpt-4.1',
  provider: 'openai',
  assistant: {
    name: 'Assistant',
    systemPrompt: 'You are helpful.',
    allowedBuiltInServiceAliases: [],
  },
};

const mockAgentSession = {
  session: mockSession,
  executionMode: 'normal' as ExecutionMode,
  yoloModeEnabled: false,
  unsafeModeEnabled: false,
  setExecutionMode: mocks.setExecutionMode,
  updateSessionConfig: mocks.updateSessionConfig,
};

const mockAgentChat = {
  workflowStatus: 'idle' as WorkflowStatus,
  error: null,
  llmError: null,
  retryMessage: mocks.retryMessage,
  resume: mocks.resume,
};

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSession: () => mockAgentSession,
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => mockAgentChat,
}));

vi.mock('@/context/LLMServiceContext', () => ({
  useLLMService: () => ({
    isCompacting: vi.fn(() => false),
    isAwaitingCompact: vi.fn(() => false),
    getCompactionPressure: vi.fn(() => undefined),
  }),
}));

vi.mock('@/context/SettingsContext', () => ({
  useSettings: () => ({
    value: {
      contextStrategy: 'default',
    },
  }),
}));

vi.mock('@/hooks/use-agent-tools', () => ({
  useAgentTools: () => ({
    availableTools: [],
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/hooks/use-token-metrics', () => ({
  useTokenMetrics: () => ({
    metrics: tokenMetricsState.metrics,
  }),
}));

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => viewportState.isMobile,
}));

interface MockAgentModelPickerProps {
  currentModel?: string;
  currentProvider?: string;
  disabled?: boolean;
  onConfigUpdate?: (model: string, provider: string) => void;
}

vi.mock('@/features/agent/components/AgentModelPicker', () => ({
  AgentModelPicker: ({
    currentModel,
    currentProvider,
    disabled,
    onConfigUpdate,
  }: MockAgentModelPickerProps) => (
    <button
      type="button"
      data-testid="model-picker"
      data-model={currentModel}
      data-provider={currentProvider}
      data-disabled={disabled ? 'true' : 'false'}
      disabled={disabled}
      onClick={() => onConfigUpdate?.('claude-3-7-sonnet', 'anthropic')}
    >
      mock-model-picker
    </button>
  ),
}));

vi.mock('@/components/ui', () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('../AgentToolsModal', () => ({
  default: () => null,
}));

vi.mock('../TokenMetricsBadge', () => ({
  TokenMetricsBadge: ({
    usage,
    compact,
    compactionPressure,
  }: {
    usage: TokenUsage;
    compact?: boolean;
    compactionPressure?: unknown;
  }) => {
    metricsBadgeState.usage = usage;
    metricsBadgeState.compact = compact ?? false;
    metricsBadgeState.hasCompactionPressure = compactionPressure !== undefined;
    return (
      <div
        data-testid="metrics-badge"
        data-compact={compact ? 'true' : 'false'}
        data-has-pressure={compactionPressure ? 'true' : 'false'}
      />
    );
  },
}));

vi.mock('@/components/ui/LoadingSpinner', () => ({
  default: () => <div>spinner</div>,
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: mocks.safeInvoke,
}));

vi.mock('@/lib/assistant/runtime-builtins', () => ({
  enforceRuntimeBuiltinAliases: (aliases: string[]) => aliases,
}));

vi.mock('@/lib/tool-call-utils', () => ({
  isBuiltinTool: () => false,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: mocks.toastSuccess,
    error: mocks.toastError,
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultValueOrOptions?: string | Record<string, unknown>,
    ) =>
      typeof defaultValueOrOptions === 'string'
        ? defaultValueOrOptions
        : key,
  }),
}));

describe('AgentChatStatusBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    viewportState.isMobile = false;
    metricsBadgeState.usage = null;
    metricsBadgeState.compact = false;
    metricsBadgeState.hasCompactionPressure = false;
    tokenMetricsState.metrics = null;
    mockAgentSession.session = { ...mockSession };
    mockAgentSession.executionMode = 'normal';
    mockAgentSession.yoloModeEnabled = false;
    mockAgentSession.unsafeModeEnabled = false;
    mockAgentChat.workflowStatus = 'idle';
    mockAgentChat.error = null;
    mockAgentChat.llmError = null;
    mocks.safeInvoke.mockResolvedValue({
      success: true,
      message: 'updated',
    });
  });

  it('keeps the model picker enabled in error state for recovery', () => {
    mockAgentChat.workflowStatus = 'error';

    render(<AgentChatStatusBar />);

    expect(screen.getByTestId('model-picker')).toHaveAttribute(
      'data-disabled',
      'false',
    );
  });

  it('keeps the model picker enabled while the workflow is paused', () => {
    mockAgentChat.workflowStatus = 'paused';

    render(<AgentChatStatusBar />);

    expect(screen.getByTestId('model-picker')).toHaveAttribute(
      'data-disabled',
      'false',
    );
  });

  it('disables the model picker while the workflow is busy', () => {
    mockAgentChat.workflowStatus = 'busy';

    render(<AgentChatStatusBar />);

    expect(screen.getByTestId('model-picker')).toHaveAttribute(
      'data-disabled',
      'true',
    );
  });

  it('renders a single execution mode control and switches to unsafe mode', () => {
    render(<AgentChatStatusBar />);

    expect(screen.getByTestId('execution-mode-control')).toBeInTheDocument();
    expect(screen.queryByText('YOLO Mode')).not.toBeInTheDocument();
    expect(screen.queryByText('Unsafe Mode')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /unsafe/i }));

    expect(mocks.setExecutionMode).toHaveBeenCalledWith('unsafe');
  });

  it('explains that hard approvals still require manual confirmation in YOLO mode', () => {
    mockAgentSession.executionMode = 'yolo';
    mockAgentSession.yoloModeEnabled = true;

    render(<AgentChatStatusBar />);

    expect(
      screen.getByTitle(/hard approvals still require manual approval/i),
    ).toBeInTheDocument();
  });

  it('updates provider and model during error recovery', async () => {
    mockAgentChat.workflowStatus = 'error';

    render(<AgentChatStatusBar />);
    fireEvent.click(screen.getByTestId('model-picker'));

    await waitFor(() => {
      expect(mocks.safeInvoke).toHaveBeenCalledWith('agent_update_session_config', {
        request: {
          sessionId: 'session-123',
          model: 'claude-3-7-sonnet',
          provider: 'anthropic',
          agentConfig: {
            ...mockSession.assistant,
            allowedBuiltInServiceAliases: [],
          },
        },
      });
    });

    expect(mocks.updateSessionConfig).toHaveBeenCalledWith(
      'claude-3-7-sonnet',
      'anthropic',
    );
    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      'Model updated. Retry to recover the session.',
    );
  });

  it('renders a compact metrics badge without compaction pressure on mobile', () => {
    viewportState.isMobile = true;
    tokenMetricsState.metrics = {
      promptTokens: 120,
      completionTokens: 45,
      totalTokens: 165,
      details: {
        evalDuration: 321,
      },
    };

    render(<AgentChatStatusBar />);

    expect(screen.getByTestId('metrics-badge')).toHaveAttribute(
      'data-compact',
      'true',
    );
    expect(screen.getByTestId('metrics-badge')).toHaveAttribute(
      'data-has-pressure',
      'false',
    );
  });
});

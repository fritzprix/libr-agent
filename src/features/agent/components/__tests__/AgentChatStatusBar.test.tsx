import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AgentChatStatusBar } from '../AgentChatStatusBar';

const mocks = vi.hoisted(() => ({
  safeInvoke: vi.fn(),
  updateSessionConfig: vi.fn(),
  toggleYoloMode: vi.fn(),
  retryMessage: vi.fn(),
  resume: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
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
  yoloModeEnabled: false,
  toggleYoloMode: mocks.toggleYoloMode,
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
    metrics: null,
  }),
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

vi.mock('./TokenMetricsBadge', () => ({
  TokenMetricsBadge: () => null,
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
    mockAgentSession.session = { ...mockSession };
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
});

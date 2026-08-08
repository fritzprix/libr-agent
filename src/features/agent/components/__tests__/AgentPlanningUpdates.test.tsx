import { render } from '@testing-library/react';
import '@testing-library/jest-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServiceContext } from '@/models/service-context';
import { AgentPlanningUpdates } from '../AgentPlanningUpdates';

const mockMarkPanelAttention = vi.fn();
const mockClearPanelAttention = vi.fn();
const mockUpdateServiceContexts = vi.fn();
const mockToast = vi.fn();

const mocks = vi.hoisted(() => ({
  sessionId: 'session-a' as string | undefined,
  showPlanningPanel: false,
  serviceContexts: {} as Record<string, ServiceContext>,
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: mocks.sessionId ? { id: mocks.sessionId } : undefined,
  }),
}));

vi.mock('@/context/AgentPlanningContext', () => ({
  useAgentPlanning: () => ({
    showPlanningPanel: mocks.showPlanningPanel,
  }),
}));

vi.mock('@/context/AgentPanelsContext', () => ({
  useAgentPanels: () => ({
    markPanelAttention: mockMarkPanelAttention,
    clearPanelAttention: mockClearPanelAttention,
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    serviceContexts: mocks.serviceContexts,
    updateServiceContexts: mockUpdateServiceContexts,
  }),
}));

vi.mock('@/hooks/use-agent-message-trigger', () => ({
  useAgentMessageTrigger: vi.fn(),
}));

vi.mock('sonner', () => ({
  toast: (...args: unknown[]) => mockToast(...args),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

function planningContext(
  goal: string | null,
  todos: Array<{ id: number; title: string; checked: boolean }> = [],
): ServiceContext {
  return {
    contextPrompt: '',
    structuredState: {
      goal,
      todos,
    },
  };
}

describe('AgentPlanningUpdates', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sessionId = 'session-a';
    mocks.showPlanningPanel = false;
    mocks.serviceContexts = {
      planning: planningContext('Ship it', [
        { id: 1, title: 'Task', checked: false },
      ]),
    };
  });

  it('clears attention and does not re-mark when switching to an empty plan session', () => {
    const { rerender } = render(<AgentPlanningUpdates />);

    expect(mockClearPanelAttention).toHaveBeenCalledWith('planning');
    mockClearPanelAttention.mockClear();
    mockMarkPanelAttention.mockClear();

    mocks.sessionId = 'session-b';
    mocks.serviceContexts = {};
    rerender(<AgentPlanningUpdates />);

    expect(mockClearPanelAttention).toHaveBeenCalledWith('planning');
    expect(mockMarkPanelAttention).not.toHaveBeenCalled();

    // Post-switch context reload settles on an empty plan.
    mocks.serviceContexts = {
      planning: planningContext(null, []),
    };
    rerender(<AgentPlanningUpdates />);

    expect(mockMarkPanelAttention).not.toHaveBeenCalled();
    expect(mockToast).not.toHaveBeenCalled();
  });

  it('absorbs the first post-switch plan reload without marking attention', () => {
    const { rerender } = render(<AgentPlanningUpdates />);
    mockClearPanelAttention.mockClear();
    mockMarkPanelAttention.mockClear();

    mocks.sessionId = 'session-b';
    mocks.serviceContexts = {};
    rerender(<AgentPlanningUpdates />);

    mocks.serviceContexts = {
      planning: planningContext('Existing goal', [
        { id: 1, title: 'Existing', checked: false },
      ]),
    };
    rerender(<AgentPlanningUpdates />);

    expect(mockMarkPanelAttention).not.toHaveBeenCalled();

    // A later in-session update should still notify.
    mocks.serviceContexts = {
      planning: planningContext('Existing goal', [
        { id: 1, title: 'Existing', checked: true },
      ]),
    };
    rerender(<AgentPlanningUpdates />);

    expect(mockMarkPanelAttention).toHaveBeenCalledWith('planning');
  });
});

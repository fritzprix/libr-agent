import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import { AgentChatHeader } from '../AgentChatHeader';

const mockRenameSession = vi.fn();
const mockToggleBookmark = vi.fn();
const mockCopyToClipboard = vi.fn();
const mockToggleShell = vi.fn();
const mockOpenPanel = vi.fn();
const mockHasPanelAttention = vi.fn(
  (/* eslint-disable @typescript-eslint/no-unused-vars */ _id: string) => false,
);
const mockIsPanelOpen = vi.fn(() => false);
let shellOpen = false;

vi.mock('@/lib/analytics', () => ({
  trackBadgeClicked: vi.fn(),
  trackPanelAction: vi.fn(),
  trackPanelViewed: vi.fn(),
  trackShortcutUsed: vi.fn(),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: {
      id: 'session-123',
      name: 'Current Session',
      status: 'idle',
      model: 'gpt-4',
      provider: 'openai',
      createdAt: new Date(),
      isBookmarked: false,
      executionMode: 'normal',
    },
  }),
  useOptionalAgentSessionState: () => ({
    session: {
      id: 'session-123',
      name: 'Current Session',
      status: 'idle',
      model: 'gpt-4',
      provider: 'openai',
      createdAt: new Date(),
      isBookmarked: false,
      executionMode: 'normal',
    },
  }),
  useAgentSessionActions: () => ({
    renameSession: mockRenameSession,
  }),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    toggleBookmark: mockToggleBookmark,
  }),
  useAgentSessionListState: () => ({
    sessions: [
      {
        id: 'session-123',
        name: 'Current Session',
        status: 'idle',
        model: 'gpt-4',
        provider: 'openai',
        createdAt: new Date(),
        isBookmarked: true,
        executionMode: 'normal',
      },
    ],
    notificationSessions: [],
  }),
}));

vi.mock('@/context/AgentPanelsContext', async () => {
  const actual = await vi.importActual<
    typeof import('@/context/AgentPanelsContext')
  >('@/context/AgentPanelsContext');
  return {
    ...actual,
    useAgentPanels: () => ({
      isShellOpen: () => shellOpen,
      openShell: vi.fn(),
      closeShell: vi.fn(),
      toggleShell: mockToggleShell,
      activeTab: 'workspace',
      setActiveTab: vi.fn(),
      isPanelOpen: mockIsPanelOpen,
      openPanel: mockOpenPanel,
      closePanel: vi.fn(),
      togglePanel: vi.fn(),
      closeAllPanels: vi.fn(),
      getLastClosedAt: () => 0,
      hasPanelAttention: mockHasPanelAttention,
      markPanelAttention: vi.fn(),
      clearPanelAttention: vi.fn(),
    }),
  };
});

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    messages: [],
  }),
}));

vi.mock('@/components/shared/SessionFilesPopover', () => ({
  SessionFilesPopover: ({ sessionId }: { sessionId: string }) => (
    <div data-testid="session-files-popover">{sessionId}</div>
  ),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copyToClipboard: mockCopyToClipboard,
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultValue?: string,
      options?: Record<string, unknown>,
    ) => {
      if (key === 'sessionHistory.actions.unbookmarkAria') {
        return 'Remove bookmark';
      }
      if (key === 'sessionHistory.actions.unbookmark') {
        return 'Remove bookmark';
      }
      if (key === 'sessionHistory.actions.bookmarkAria') {
        return 'Bookmark session';
      }
      if (key === 'sessionHistory.actions.bookmark') {
        return 'Bookmark';
      }
      if (key === 'agent.header.sessionLabel') {
        return 'Session';
      }
      if (key === 'agent.header.renameAria') {
        return 'Rename session';
      }
      if (key === 'agent.header.copyAria') {
        return 'Copy conversation';
      }
      if (key === 'agent.header.toggleShellAria') {
        return 'Toggle agent panels';
      }
      if (key === 'agent.header.toggleShellHasUpdatesAria') {
        return 'Toggle agent panels (has updates)';
      }
      if (key === 'agent.header.statusBadgesAria') {
        return 'Panel update badges';
      }
      if (key === 'agent.header.panelBadgeAria') {
        const panel =
          typeof options?.panel === 'string'
            ? options.panel
            : typeof defaultValue === 'string'
              ? ''
              : '';
        // i18next mock: defaultValue is template; options carries interpolations
        const interpolatedPanel =
          typeof options?.panel === 'string' ? options.panel : panel;
        return `${interpolatedPanel} has updates — click to open`;
      }
      if (key === 'agent.header.panelBadgeSrOnly') {
        return 'has updates';
      }
      if (key === 'agent.header.panelBadgeLive') {
        const panel =
          typeof options?.panel === 'string' ? options.panel : '';
        return `${panel} has updates`;
      }
      if (key === 'agent.processes.title') {
        return 'Processes';
      }
      if (key === 'agent.planning.title') {
        return 'Planning';
      }
      if (key === 'agent.workspace.title') {
        return 'Workspace';
      }
      if (key === 'agent.header.defaultAssistant') {
        return 'Agent';
      }
      if (key === 'sessionHistory.actions.viewAria') {
        return `View session ${options?.name ?? ''}`;
      }
      return defaultValue ?? key;
    },
  }),
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('AgentChatHeader', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    shellOpen = false;
    mockHasPanelAttention.mockReturnValue(false);
    mockIsPanelOpen.mockReturnValue(false);
  });

  it('renders a bookmark toggle for the active chat session and wires it to list actions', () => {
    render(<AgentChatHeader />);

    fireEvent.click(screen.getByRole('button', { name: 'Remove bookmark' }));

    expect(mockToggleBookmark).toHaveBeenCalledWith('session-123');
    expect(screen.queryByText('(Agent)')).not.toBeInTheDocument();
  });

  it('places the rename action before the bookmark action in the title cluster', () => {
    render(<AgentChatHeader />);

    const renameButton = screen.getByRole('button', { name: 'Rename session' });
    const bookmarkButton = screen.getByRole('button', {
      name: 'Remove bookmark',
    });

    expect(
      renameButton.compareDocumentPosition(bookmarkButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('toggles the agent panel shell from the header control', () => {
    render(<AgentChatHeader />);

    fireEvent.click(
      screen.getByRole('button', { name: 'Toggle agent panels' }),
    );

    expect(mockToggleShell).toHaveBeenCalledTimes(1);
  });

  it('shows an attention dot on the shell toggle when any tab is marked', () => {
    mockHasPanelAttention.mockImplementation(
      (id: string) => id === 'processes',
    );

    render(<AgentChatHeader />);

    expect(screen.getByTestId('panel-attention-dot')).toBeInTheDocument();
    expect(
      screen.getByRole('button', {
        name: 'Toggle agent panels (has updates)',
      }),
    ).toBeInTheDocument();
  });

  it('shows a processes status badge when that panel has attention and opens it on click', async () => {
    const { trackBadgeClicked } = await import('@/lib/analytics');
    mockHasPanelAttention.mockImplementation(
      (id: string) => id === 'processes',
    );

    render(<AgentChatHeader />);

    const badge = screen.getByTestId('header-status-badge-processes');
    expect(badge).toHaveAttribute('aria-controls', 'agent-side-panel-shell');
    expect(badge).toHaveAttribute(
      'aria-label',
      'Processes has updates — click to open',
    );

    fireEvent.click(badge);

    expect(trackBadgeClicked).toHaveBeenCalledWith('processes', 'session-123');
    expect(mockOpenPanel).toHaveBeenCalledWith('processes');
  });

  it('hides status badges when no panel has attention', () => {
    render(<AgentChatHeader />);

    expect(
      screen.queryByTestId('header-status-badges'),
    ).not.toBeInTheDocument();
  });
});

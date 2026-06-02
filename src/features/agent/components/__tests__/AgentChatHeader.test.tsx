import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import { AgentChatHeader } from '../AgentChatHeader';

const mockRenameSession = vi.fn();
const mockToggleBookmark = vi.fn();
const mockCopyToClipboard = vi.fn();
const mockTogglePlanningPanel = vi.fn();
const mockToggleWorkspacePanel = vi.fn();

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
      yoloMode: false,
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
      yoloMode: false,
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
        yoloMode: false,
      },
    ],
    notificationSessions: [],
  }),
}));

vi.mock('@/context/AgentPlanningContext', () => ({
  useAgentPlanning: () => ({
    showPlanningPanel: false,
    togglePlanningPanel: mockTogglePlanningPanel,
  }),
}));

vi.mock('@/context/AgentWorkspaceContext', () => ({
  useAgentWorkspace: () => ({
    showWorkspacePanel: false,
    toggleWorkspacePanel: mockToggleWorkspacePanel,
  }),
}));

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
      if (key === 'agent.header.toggleWorkspaceAria') {
        return 'Toggle workspace';
      }
      if (key === 'agent.header.togglePlanningAria') {
        return 'Toggle planning';
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
});

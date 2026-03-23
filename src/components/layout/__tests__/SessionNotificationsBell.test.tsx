import { fireEvent, render, screen } from '@testing-library/react';
import { waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { SessionNotificationsBell } from '../SessionNotificationsBell';
import type { AgentSession } from '@/models/agent';

const mockNavigate = vi.fn();
const mockMarkSessionViewed = vi.fn().mockResolvedValue(undefined);

const notificationSession: AgentSession = {
  id: 'session-target',
  name: 'Target Session',
  status: 'idle',
  model: 'gpt-5.4',
  provider: 'openai',
  createdAt: new Date('2026-03-22T00:00:00.000Z'),
  updatedAt: new Date('2026-03-22T00:10:00.000Z'),
  assistant: undefined,
  yoloMode: false,
  lastAttentionAt: new Date('2026-03-22T00:10:00.000Z'),
  lastAttentionReason: 'recurringStop',
  pendingApprovalCount: 1,
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) => {
      if (key === 'notifications.pendingApprovals') {
        return `Pending approvals: ${options?.count ?? 0}`;
      }
      return key;
    },
  }),
}));

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/components/ui/dropdown-menu', () => ({
  DropdownMenu: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
    <div role="menu">{children}</div>
  ),
  DropdownMenuLabel: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuItem: ({
    children,
    onSelect,
  }: {
    children: React.ReactNode;
    onSelect?: (event: Event) => void;
  }) => (
    <button
      type="button"
      role="menuitem"
      onClick={() =>
        onSelect?.({
          preventDefault() {},
          stopPropagation() {},
        } as Event)
      }
    >
      {children}
    </button>
  ),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    markSessionViewed: mockMarkSessionViewed,
  }),
  useAgentSessionListState: () => ({
    notificationSessions: [notificationSession],
    unreadNotificationCount: 1,
  }),
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('SessionNotificationsBell', () => {
  beforeEach(() => {
    mockNavigate.mockClear();
    mockMarkSessionViewed.mockClear();
    mockMarkSessionViewed.mockResolvedValue(undefined);
  });

  it('navigates to the clicked notification session', async () => {
    render(<SessionNotificationsBell />);

    fireEvent.click(
      screen.getByRole('button', { name: 'notifications.open' }),
    );

    const item = await screen.findByRole('menuitem', {
      name: /Target Session/i,
    });
    fireEvent.click(item);

    expect(mockMarkSessionViewed).toHaveBeenCalledWith('session-target');
    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith('/agent/session-target');
    });
  });
});

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { SessionNotificationsBell } from '../SessionNotificationsBell';
import type { AgentSession } from '@/models/agent';

const mockNavigate = vi.fn();
const mockMarkSessionViewed = vi.fn().mockResolvedValue(undefined);
const mockMarkAllNotificationsViewed = vi.fn().mockResolvedValue(undefined);

const notificationSession1: AgentSession = {
  id: 'session-target',
  name: 'Target Session',
  status: 'idle',
  model: 'gpt-5.4',
  provider: 'openai',
  createdAt: new Date('2026-03-22T00:00:00.000Z'),
  updatedAt: new Date('2026-03-22T00:10:00.000Z'),
  assistant: undefined,
  executionMode: 'normal',
  lastAttentionAt: new Date('2026-03-22T00:10:00.000Z'),
  lastAttentionReason: 'recurringStop',
  pendingApprovalCount: 0,
};

const notificationSession2: AgentSession = {
  id: 'session-approval',
  name: 'Approval Session',
  status: 'idle',
  model: 'gpt-5.4',
  provider: 'openai',
  createdAt: new Date('2026-03-22T00:00:00.000Z'),
  updatedAt: new Date('2026-03-22T00:10:00.000Z'),
  assistant: undefined,
  executionMode: 'normal',
  lastAttentionAt: new Date('2026-03-22T00:10:00.000Z'),
  lastAttentionReason: 'recurringStop',
  pendingApprovalCount: 2,
};

let mockNotificationSessions: AgentSession[] = [
  notificationSession1,
  notificationSession2,
];

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
  DropdownMenuTrigger: React.forwardRef(function DropdownMenuTrigger(
    { children }: { children: React.ReactNode },
    ref: React.Ref<HTMLDivElement>,
  ) {
    return <div ref={ref}>{children}</div>;
  }),
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
    <div
      role="menuitem"
      onClick={(event) => {
        if (!event.defaultPrevented) {
          onSelect?.({
            preventDefault() {},
            stopPropagation() {},
          } as Event);
        }
      }}
    >
      {children}
    </div>
  ),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    markSessionViewed: mockMarkSessionViewed,
    markAllNotificationsViewed: mockMarkAllNotificationsViewed,
  }),
  useAgentSessionListState: () => ({
    notificationSessions: mockNotificationSessions,
    unreadNotificationCount: mockNotificationSessions.length,
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
    mockMarkAllNotificationsViewed.mockClear();
    mockMarkAllNotificationsViewed.mockResolvedValue(undefined);
    mockNotificationSessions = [notificationSession1, notificationSession2];
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

  it('dismisses a single notification without navigation', async () => {
    render(<SessionNotificationsBell />);

    fireEvent.click(
      screen.getByRole('button', { name: 'notifications.open' }),
    );

    const dismissButtons = screen.getAllByRole('button', {
      name: 'notifications.dismiss',
    });
    expect(dismissButtons).toHaveLength(2);

    fireEvent.click(dismissButtons[0]);

    expect(mockMarkSessionViewed).toHaveBeenCalledWith('session-target');
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('calls markAllNotificationsViewed when "Mark all read" is clicked and skips pending approval sessions', async () => {
    mockMarkAllNotificationsViewed.mockImplementation(async () => {
      const targets = mockNotificationSessions.filter(
        (session) => (session.pendingApprovalCount ?? 0) <= 0,
      );
      await Promise.allSettled(
        targets.map((session) => mockMarkSessionViewed(session.id)),
      );
    });

    render(<SessionNotificationsBell />);

    fireEvent.click(
      screen.getByRole('button', { name: 'notifications.open' }),
    );

    const markAllReadButton = screen.getByRole('button', {
      name: 'notifications.markAllRead',
    });
    fireEvent.click(markAllReadButton);

    expect(mockMarkAllNotificationsViewed).toHaveBeenCalledTimes(1);
    expect(mockMarkSessionViewed).toHaveBeenCalledWith('session-target');
    expect(mockMarkSessionViewed).not.toHaveBeenCalledWith('session-approval');
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('does not show "Mark all read" when notification list is empty', async () => {
    mockNotificationSessions = [];
    render(<SessionNotificationsBell />);

    fireEvent.click(
      screen.getByRole('button', { name: 'notifications.open' }),
    );

    expect(
      screen.queryByRole('button', { name: 'notifications.markAllRead' }),
    ).not.toBeInTheDocument();
  });
});

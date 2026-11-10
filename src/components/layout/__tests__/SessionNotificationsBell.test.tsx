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
  // Mirrors @radix-ui/react-menu MenuItem (2.1.15). The real item records
  // `isPointerDownRef` in its own onPointerDown and, in onPointerUp, synthesizes
  // a click on itself when the matching pointerdown never reached it. That
  // synthesis is the regression this suite must guard against, so the mock has
  // to reproduce it instead of being a bare onClick div.
  DropdownMenuItem: ({
    children,
    onSelect,
    onPointerDown,
    onPointerUp,
    onClick,
    'aria-label': ariaLabel,
  }: {
    children: React.ReactNode;
    'aria-label'?: string;
    onSelect?: (event: Event) => void;
    onPointerDown?: React.PointerEventHandler<HTMLDivElement>;
    onPointerUp?: React.PointerEventHandler<HTMLDivElement>;
    onClick?: React.MouseEventHandler<HTMLDivElement>;
  }) => {
    const isPointerDownRef = React.useRef(false);
    const itemRef = React.useRef<HTMLDivElement>(null);

    return (
      <div
        ref={itemRef}
        role="menuitem"
        aria-label={ariaLabel}
        onPointerDown={(event) => {
          onPointerDown?.(event);
          isPointerDownRef.current = true;
        }}
        onPointerUp={(event) => {
          onPointerUp?.(event);
          if (!isPointerDownRef.current) itemRef.current?.click();
        }}
        onClick={(event) => {
          onClick?.(event);
          if (event.defaultPrevented) return;
          onSelect?.(
            new Event('menu.itemSelect', { bubbles: true, cancelable: true }),
          );
        }}
      >
        {children}
      </div>
    );
  },
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

  it('dismisses a single notification without navigating on a real pointer click', async () => {
    render(<SessionNotificationsBell />);

    fireEvent.click(
      screen.getByRole('button', { name: 'notifications.open' }),
    );

    const dismissButtons = screen.getAllByRole('menuitem', {
      name: 'notifications.dismiss',
    });
    expect(dismissButtons).toHaveLength(2);

    // A real mouse click is pointerdown -> pointerup -> click. Radix MenuItem
    // synthesizes its own click on the row in onPointerUp when the pointerdown
    // never reached it, so a bare fireEvent.click cannot detect the regression
    // in #2507 (dismiss must not open the session).
    fireEvent.pointerDown(dismissButtons[0]);
    fireEvent.pointerUp(dismissButtons[0]);
    fireEvent.click(dismissButtons[0]);

    expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
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

    const markAllReadButton = screen.getByRole('menuitem', {
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
      screen.queryByRole('menuitem', { name: 'notifications.markAllRead' }),
    ).not.toBeInTheDocument();
  });
});

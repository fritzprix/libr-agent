/**
 * Keyboard-only use of the notification bell with the real Radix menu (#2537).
 * The sibling suite mocks the menu; this one keeps @radix-ui/react-menu so its
 * roving focus (arrow keys only, Tab blocked) is what the test exercises.
 */
import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { SessionNotificationsBell } from '../SessionNotificationsBell';
import type { AgentSession } from '@/models/agent';

const mockNavigate = vi.fn();
const mockMarkSessionViewed = vi.fn().mockResolvedValue(undefined);
const mockMarkAllNotificationsViewed = vi.fn().mockResolvedValue(undefined);

function session(id: string, name: string): AgentSession {
  return {
    id,
    name,
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
}

const sessions = [session('session-a', 'Alpha'), session('session-b', 'Beta')];

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  TooltipContent: () => null,
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    markSessionViewed: mockMarkSessionViewed,
    markAllNotificationsViewed: mockMarkAllNotificationsViewed,
  }),
  useAgentSessionListState: () => ({
    notificationSessions: sessions,
    unreadNotificationCount: sessions.length,
  }),
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

async function openWithKeyboard() {
  const trigger = screen.getByRole('button', { name: 'notifications.open' });
  trigger.focus();
  fireEvent.keyDown(trigger, { key: 'Enter' });
  const menu = await screen.findByRole('menu');
  await waitFor(() => expect(menu).toContainElement(document.activeElement as HTMLElement));
  return menu;
}

/**
 * Press ArrowDown until the focused element matches, like a keyboard user.
 * Radix roving focus moves focus in a setTimeout, so wait a tick per press.
 */
async function arrowTo(name: RegExp | string) {
  for (let i = 0; i < 10; i += 1) {
    const active = document.activeElement as HTMLElement;
    const label = active.getAttribute('aria-label') ?? active.textContent ?? '';
    if (typeof name === 'string' ? label === name : name.test(label)) return active;
    await act(async () => {
      fireEvent.keyDown(active, { key: 'ArrowDown' });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
  throw new Error(`could not reach ${String(name)} with ArrowDown`);
}

describe('SessionNotificationsBell keyboard access (#2537)', () => {
  beforeEach(() => {
    mockNavigate.mockClear();
    mockMarkSessionViewed.mockClear();
    mockMarkAllNotificationsViewed.mockClear();
  });

  it('nests no interactive control inside a menu item', async () => {
    render(<SessionNotificationsBell />);
    const menu = await openWithKeyboard();

    expect(menu.querySelectorAll('[role="menuitem"] button')).toHaveLength(0);
    expect(
      screen.getByRole('menuitem', { name: 'notifications.markAllRead' }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole('menuitem', { name: 'notifications.dismiss' }),
    ).toHaveLength(2);
  });

  it('dismisses one notification with arrow keys and Enter, without navigating', async () => {
    render(<SessionNotificationsBell />);
    await openWithKeyboard();

    const dismiss = await arrowTo('notifications.dismiss');
    expect(dismiss).toHaveFocus();
    await act(async () => {
      fireEvent.keyDown(dismiss, { key: 'Enter' });
    });

    expect(mockMarkSessionViewed).toHaveBeenCalledWith('session-a');
    expect(mockNavigate).not.toHaveBeenCalled();
    // The menu stays open for the next action.
    expect(screen.getByRole('menu')).toBeInTheDocument();
  });

  it('marks all notifications read with the keyboard', async () => {
    render(<SessionNotificationsBell />);
    await openWithKeyboard();

    const markAll = await arrowTo('notifications.markAllRead');
    await act(async () => {
      fireEvent.keyDown(markAll, { key: 'Enter' });
    });

    expect(mockMarkAllNotificationsViewed).toHaveBeenCalledTimes(1);
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

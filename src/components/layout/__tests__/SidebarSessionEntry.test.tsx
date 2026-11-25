import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { SidebarProvider } from '@/components/ui/sidebar';
import type { AgentSession } from '@/models/agent';
import { SidebarSessionEntry } from '../SidebarSessionEntry';

vi.mock('@/hooks/use-mobile', () => ({ useIsMobile: () => false }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultValue?: string,
      options?: Record<string, string>,
    ) => {
      const text = typeof defaultValue === 'string' ? defaultValue : key;
      return text.replace(
        /\{\{(\w+)\}\}/g,
        (_match, name: string) => options?.[name] ?? '',
      );
    },
  }),
}));

function createSession(overrides: Partial<AgentSession> = {}): AgentSession {
  return {
    id: 'session-1234567890',
    name: 'Quarterly report',
    status: 'idle',
    model: 'test-model',
    provider: 'test-provider',
    createdAt: new Date('2026-03-20T00:00:00Z'),
    updatedAt: new Date('2026-03-20T00:00:00Z'),
    executionMode: 'normal',
    ...overrides,
  };
}

function renderEntry(session: AgentSession) {
  return render(
    <MemoryRouter>
      <SidebarProvider>
        <SidebarSessionEntry session={session} isActive={false} />
      </SidebarProvider>
    </MemoryRouter>,
  );
}

describe('SidebarSessionEntry', () => {
  it('links to the session when it is not open elsewhere', () => {
    renderEntry(createSession());

    const link = screen.getByRole('link');
    expect(link).toHaveAttribute('href', '/agent/session-1234567890');
    expect(screen.queryByText('Open elsewhere')).not.toBeInTheDocument();
  });

  it('shows the badge and refuses entry when the session is open elsewhere', () => {
    renderEntry(createSession({ openElsewhere: true }));

    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    expect(screen.getByText('Open elsewhere')).toBeInTheDocument();
    const button = screen.getByRole('button', {
      name: 'Session Quarterly report is open in another process',
    });
    expect(button).toBeDisabled();
  });
});

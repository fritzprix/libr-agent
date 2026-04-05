import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AgentSession } from '@/models/agent';
import Org from '../Org';

const mockNavigate = vi.fn();
const mockLoadSessions = vi.fn();

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultString?: string, options?: { count?: number }) => {
      if (typeof options?.count === 'number' && defaultString) {
        return defaultString.replace('{{count}}', String(options.count));
      }
      return defaultString || _key;
    },
  }),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListState: () => ({
    sessions: [
      {
        id: 'solo',
        name: 'Solo Session',
        status: 'idle',
        model: 'test-model',
        provider: 'test-provider',
        createdAt: new Date('2026-04-05T00:00:00Z'),
        lineageId: 'lineage-1',
        yoloMode: false,
      },
      {
        id: 'root',
        name: 'Org Root',
        status: 'idle',
        model: 'test-model',
        provider: 'test-provider',
        createdAt: new Date('2026-04-05T00:00:00Z'),
        orgId: 'org-1',
        orgName: 'Research Org',
        orgRootSessionId: 'root',
        yoloMode: false,
      },
      {
        id: 'child',
        name: 'Org Child',
        status: 'busy',
        model: 'test-model',
        provider: 'test-provider',
        createdAt: new Date('2026-04-05T00:01:00Z'),
        parentSessionId: 'root',
        lineageId: 'lineage-2',
        depth: 1,
        orgId: 'org-1',
        orgName: 'Research Org',
        orgRootSessionId: 'root',
        yoloMode: false,
      },
    ] satisfies AgentSession[],
    isSessionsListLoading: false,
  }),
  useAgentSessionListActions: () => ({
    loadSessions: mockLoadSessions,
  }),
}));

describe('Org', () => {
  it('renders explicit org cards only and resumes the root session', () => {
    render(<Org />);

    expect(screen.getByText('Org View')).toBeInTheDocument();
    expect(screen.getByText('Research Org')).toBeInTheDocument();
    expect(screen.getByText(/2 members/)).toBeInTheDocument();
    expect(screen.queryByText('Solo Session')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Resume Root Session' }));

    expect(mockNavigate).toHaveBeenCalledWith('/agent/root');
  });
});

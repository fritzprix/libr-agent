import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import Org from '../Org';

const mockNavigate = vi.fn();
const mockDeleteSession = vi.fn();

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

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn().mockResolvedValue([
    {
      id: 'solo',
      name: 'Solo Session',
      status: 'idle',
      model: 'test-model',
      provider: 'test-provider',
      createdAt: new Date('2026-04-05T00:00:00Z').getTime(),
      updatedAt: new Date('2026-04-05T00:00:00Z').getTime(),
      lineageId: 'lineage-1',
      executionMode: 'normal',
    },
    {
      id: 'root',
      name: 'Org Root',
      status: 'idle',
      model: 'test-model',
      provider: 'test-provider',
      createdAt: new Date('2026-04-05T00:00:00Z').getTime(),
      updatedAt: new Date('2026-04-05T00:00:00Z').getTime(),
      orgId: 'org-1',
      orgName: 'Research Org',
      orgRootSessionId: 'root',
      executionMode: 'normal',
    },
    {
      id: 'child',
      name: 'Org Child',
      status: 'busy',
      model: 'test-model',
      provider: 'test-provider',
      createdAt: new Date('2026-04-05T00:01:00Z').getTime(),
      updatedAt: new Date('2026-04-05T00:01:00Z').getTime(),
      parentSessionId: 'root',
      lineageId: 'lineage-2',
      depth: 1,
      orgId: 'org-1',
      orgName: 'Research Org',
      orgRootSessionId: 'root',
      executionMode: 'normal',
    },
  ]),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    deleteSession: mockDeleteSession,
  }),
}));

describe('Org', () => {
  it('renders explicit org cards only and resumes the root session', async () => {
    render(<Org />);

    await waitFor(() => {
      expect(screen.getByText('Org View')).toBeInTheDocument();
      expect(screen.getByText('Research Org')).toBeInTheDocument();
    });
    expect(screen.getByText('Members')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.queryByText('Solo Session')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Resume Root Session' }));

    expect(mockNavigate).toHaveBeenCalledWith('/agent/root');
  });

  it('filters org cards by search query', async () => {
    render(<Org />);

    await waitFor(() => {
      expect(screen.getByText('Research Org')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('Search organizations'), {
      target: { value: 'missing-org' },
    });

    await waitFor(() => {
      expect(screen.queryByText('Research Org')).not.toBeInTheDocument();
      expect(
        screen.getByText('No organizations match "{{query}}"'),
      ).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('Clear search'));

    await waitFor(() => {
      expect(screen.getByText('Research Org')).toBeInTheDocument();
    });
  });
});

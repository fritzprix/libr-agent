import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { PlaybookCard } from '../Card';
import type { Playbook } from '@/types/playbook';
import { getAgentSessionMetadata } from '@/lib/backend/agent-commands';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (key === 'playbook.card.steps') {
        return `${opts?.count ?? 0} Steps`;
      }
      if (key === 'playbook.card.created') {
        return `Created ${opts?.date ?? ''}`;
      }
      if (key === 'playbook.card.startPinnedTooltip') {
        return `Opens “${opts?.sessionName ?? ''}”`;
      }
      if (key === 'playbook.card.pinnedBadgeTooltip') {
        return `Start reuses “${opts?.sessionName ?? ''}”`;
      }
      const labels: Record<string, string> = {
        'playbook.card.start': 'Start',
        'playbook.card.startPinned': 'Continue',
        'playbook.card.pinnedBadge': 'Pinned session',
        'playbook.card.pinnedSessionUntitled': 'Untitled session',
        'playbook.card.pinnedSessionMissing': 'Session missing',
        'playbook.card.pinnedSessionMissingTooltip':
          'Pinned session is gone — Start will create a new one',
        'playbook.card.bookmark': 'Bookmark',
        'playbook.card.deleteTooltip': 'Delete Playbook',
      };
      return labels[key] ?? key;
    },
    i18n: { language: 'en' },
  }),
}));

vi.mock('@/lib/date-utils', () => ({
  getDateFormatter: () => ({
    format: () => '1/1/2024',
  }),
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  getAgentSessionMetadata: vi.fn(),
}));

const basePlaybook: Playbook & { id: string; createdAt: Date } = {
  id: 'pb-1',
  agentId: 'agent-1',
  goal: 'Ship the feature',
  initialCommand: 'do the thing',
  workflow: [
    {
      stepId: 's1',
      description: 'step',
      action: { toolName: 't', purpose: 'p' },
      requiredData: [],
      outputVariable: 'out',
    },
  ],
  successCriteria: { description: 'done' },
  createdAt: new Date('2024-01-01T00:00:00Z'),
};

function renderCard(playbook: Playbook & { id: string; createdAt: Date }) {
  return render(
    <MemoryRouter>
      <PlaybookCard
        playbook={playbook}
        assistantName="Assistant"
        onBookmarkToggle={vi.fn()}
        onDelete={vi.fn()}
      />
    </MemoryRouter>,
  );
}

describe('PlaybookCard pinned affordance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows Start when unpinned', () => {
    renderCard(basePlaybook);
    expect(screen.getByRole('button', { name: /Start/i })).toBeInTheDocument();
    expect(screen.queryByText('Continue')).not.toBeInTheDocument();
  });

  it('shows the pinned session name, not the raw id', async () => {
    vi.mocked(getAgentSessionMetadata).mockResolvedValue({
      id: 'a1b2c3d4e5',
      name: 'Weekly report thread',
      status: 'idle',
      model: 'gpt',
      provider: 'openai',
      createdAt: 1,
      executionMode: 'yolo',
      workspaceIsolation: 'host',
    });

    renderCard({
      ...basePlaybook,
      defaultTargetSession: {
        mode: 'pin',
        sessionId: 'a1b2c3d4e5',
      },
    });

    expect(
      screen.getByRole('button', { name: /Continue/i }),
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('Weekly report thread')).toBeInTheDocument();
    });
    expect(screen.queryByText('a1b2c3d4e5')).not.toBeInTheDocument();
  });

  it('shows missing label when the pinned session is gone', async () => {
    vi.mocked(getAgentSessionMetadata).mockResolvedValue(null);

    renderCard({
      ...basePlaybook,
      defaultTargetSession: {
        mode: 'pin',
        sessionId: 'dead-session',
      },
    });

    await waitFor(() => {
      expect(screen.getByText('Session missing')).toBeInTheDocument();
    });
    expect(screen.queryByText('dead-session')).not.toBeInTheDocument();
  });
});

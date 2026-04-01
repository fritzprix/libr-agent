import { fireEvent, render, screen } from '@testing-library/react';
import { SessionHistoryPanel } from '../SessionHistoryPanel';
import type { AgentSession } from '@/models/agent';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultString: string,
      options?: Record<string, unknown>,
    ) => {
      if (key === 'sessionHistory.card.fallbackName') {
        return `Session ${String(options?.id ?? '')}`;
      }
      if (key === 'sessionHistory.lineageHint.child') {
        return `Child of ${String(options?.parentName ?? '')}`;
      }
      return defaultString || key;
    },
  }),
}));

vi.mock('../SessionCard', () => ({
  SessionCard: ({
    session,
    descendantCount,
    hasExpandableChildren,
    isExpanded,
    onToggleExpand,
    selectedLineageId,
    onLineageSelect,
  }: {
    session: AgentSession;
    descendantCount: number;
    hasExpandableChildren?: boolean;
    isExpanded?: boolean;
    onToggleExpand?: (sessionId: string) => void;
    selectedLineageId?: string | null;
    onLineageSelect?: (lineageId: string) => void;
  }) => (
    <div data-testid={`session-card-${session.id}`}>
      <span>{session.name}</span>
      <span data-testid={`descendants-${session.id}`}>{descendantCount}</span>
      {session.lineageId && (
        <button
          type="button"
          aria-pressed={selectedLineageId === session.lineageId}
          aria-label={`Filter by lineage ${session.lineageId.slice(0, 8)}`}
          onClick={() => onLineageSelect?.(session.lineageId!)}
        >
          Lineage
        </button>
      )}
      {hasExpandableChildren && (
        <button type="button" onClick={() => onToggleExpand?.(session.id)}>
          {isExpanded ? 'Collapse' : 'Expand'}
        </button>
      )}
    </div>
  ),
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

function createSession(
  id: string,
  name: string,
  overrides: Partial<AgentSession> = {},
): AgentSession {
  return {
    id,
    name,
    status: 'idle',
    model: 'test-model',
    provider: 'test-provider',
    createdAt: new Date('2026-03-19T00:00:00Z'),
    updatedAt: new Date('2026-03-19T00:00:00Z'),
    yoloMode: false,
    ...overrides,
  };
}

describe('SessionHistoryPanel', () => {
  it('correctly calculates descendant counts for a session tree', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child1', 'Child 1', { parentSessionId: 'root' }),
      createSession('child2', 'Child 2', { parentSessionId: 'root' }),
      createSession('grandchild1', 'Grandchild 1', {
        parentSessionId: 'child2',
      }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('descendants-root')).toHaveTextContent('3');
  });

  it('shows only root sessions before expansion', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', { parentSessionId: 'root' }),
      createSession('grandchild', 'Grandchild', { parentSessionId: 'child' }),
      createSession('other-root', 'Other Root'),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-other-root')).toBeInTheDocument();
    expect(screen.queryByTestId('session-card-child')).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('session-card-grandchild'),
    ).not.toBeInTheDocument();
  });

  it('expands child sessions when the root is toggled', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', { parentSessionId: 'root' }),
      createSession('grandchild', 'Grandchild', { parentSessionId: 'child' }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Expand' }));

    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();
    expect(
      screen.queryByTestId('session-card-grandchild'),
    ).not.toBeInTheDocument();
  });

  it('keeps lineage roots visible when a descendant matches the status filter', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root', { status: 'idle' }),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        status: 'error',
      }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="error"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();
  });

  it('keeps lineage roots visible in bookmarked view when only a descendant is bookmarked', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();
  });

  it('allows collapsing an auto-expanded lineage in bookmarked view', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Collapse' }));

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.queryByTestId('session-card-child')).not.toBeInTheDocument();
  });

  it('clears a selected lineage when that lineage disappears from sessions', () => {
    const lineageId = 'lineage-1';
    const initialSessions: AgentSession[] = [
      createSession('root', 'Root', { lineageId }),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        lineageId,
      }),
    ];
    const nextSessions: AgentSession[] = [
      createSession('other-root', 'Other Root', { lineageId: 'lineage-2' }),
    ];

    const { rerender } = render(
      <SessionHistoryPanel
        sessions={initialSessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'Filter by lineage lineage-' }),
    );

    expect(screen.getByText('Show all lineages')).toBeInTheDocument();

    rerender(
      <SessionHistoryPanel
        sessions={nextSessions}
        isLoading={false}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(
      screen.queryByText('Show all lineages'),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('session-card-other-root')).toBeInTheDocument();
  });

  it('keeps auto-expanded bookmarked lineage visible after rerender with equal data', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];

    const { rerender } = render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();

    rerender(
      <SessionHistoryPanel
        sessions={[
          createSession('root', 'Root'),
          createSession('child', 'Child', {
            parentSessionId: 'root',
            isBookmarked: true,
          }),
        ]}
        isLoading={false}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onActiveStatusFilterChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();
  });
});

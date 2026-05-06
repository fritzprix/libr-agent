import { fireEvent, render, screen } from '@testing-library/react';
import { SessionHistoryPanel } from '../SessionHistoryPanel';
import type { AgentSession } from '@/models/agent';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';

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
  }: {
    session: AgentSession;
    descendantCount: number;
    hasExpandableChildren?: boolean;
    isExpanded?: boolean;
    onToggleExpand?: (sessionId: string) => void;
  }) => (
    <div data-testid={`session-card-${session.id}`}>
      <span>{session.name}</span>
      <span data-testid={`descendants-${session.id}`}>{descendantCount}</span>
      {hasExpandableChildren && (
        <button type="button" onClick={() => onToggleExpand?.(session.id)}>
          {isExpanded ? 'Collapse' : 'Expand'}
        </button>
      )}
    </div>
  ),
}));

vi.mock('react-virtuoso', () => ({
  Virtuoso: ({
    data,
    itemContent,
    components,
  }: {
    data: unknown[];
    itemContent: (index: number, item: unknown) => ReactNode;
    components?: { Footer?: () => ReactNode };
  }) => (
    <div>
      {data.map((item, index) => (
        <div key={index}>{itemContent(index, item)}</div>
      ))}
      {components?.Footer ? <components.Footer /> : null}
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

const defaultProps = {
  hasMoreSessions: false,
  isLoadingMoreSessions: false,
  onActiveTabChange: () => {},
  onActiveStatusFilterChange: () => {},
  onSearchQueryChange: () => {},
  onRefresh: () => {},
  onLoadMore: () => {},
  onResume: () => {},
  onDelete: () => {},
  onDeleteOnly: () => {},
};

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
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
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
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
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
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery=""
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
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="error"
        searchQuery=""
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
        {...defaultProps}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
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
        {...defaultProps}
        activeTab="bookmarked"
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Collapse' }));

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.queryByTestId('session-card-child')).not.toBeInTheDocument();
  });

  it('resets collapsed auto-expanded lineage state when the filter context changes', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child-alpha', 'Alpha child', {
        parentSessionId: 'root',
      }),
      createSession('child-beta', 'Beta child', {
        parentSessionId: 'root',
      }),
    ];

    const { rerender } = render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery="alpha"
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Collapse' }));

    expect(
      screen.queryByTestId('session-card-child-alpha'),
    ).not.toBeInTheDocument();

    rerender(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeTab="all"
        activeStatusFilter="all"
        searchQuery="beta"
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child-beta')).toBeInTheDocument();
  });
});

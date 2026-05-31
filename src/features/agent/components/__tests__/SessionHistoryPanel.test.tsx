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
      if (defaultString && options) {
        return Object.entries(options).reduce((text, [optionKey, value]) => {
          return text.replace(`{{${optionKey}}}`, String(value));
        }, defaultString);
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
    components?: {
      List?: (props: { children: ReactNode }) => ReactNode;
      Item?: (props: { children: ReactNode }) => ReactNode;
      Footer?: () => ReactNode;
    };
  }) => (
    (() => {
      const items = data.map((item, index) => {
        const content = itemContent(index, item);
        return components?.Item ? (
          <components.Item key={index}>{content}</components.Item>
        ) : (
          <div key={index}>{content}</div>
        );
      });

      const listChildren = (
        <>
          {items}
          {components?.Footer ? <components.Footer /> : null}
        </>
      );

      return components?.List ? (
        <components.List>{listChildren}</components.List>
      ) : (
        <div>{listChildren}</div>
      );
    })()
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
  onActiveStatusFilterChange: () => {},
  onSearchQueryChange: () => {},
  onRefresh: () => {},
  onLoadMore: () => {},
  onResume: () => {},
  onDelete: () => {},
  onDeleteOnly: () => {},
  onToggleBookmark: () => {},
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

  it('renders the virtualized session list with list semantics', () => {
    render(
      <SessionHistoryPanel
        sessions={[createSession('root', 'Root')]}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    const sessionList = screen.getByRole('list');
    expect(sessionList).toHaveAttribute('aria-labelledby', 'session-heading');
    expect(screen.getAllByRole('listitem')).not.toHaveLength(0);
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
        activeStatusFilter="error"
        searchQuery=""
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child')).toBeInTheDocument();
  });

  it('renders bookmarked sessions in a dedicated top section', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];
    const onResume = vi.fn();

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        onResume={onResume}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    expect(screen.getByText('Bookmarked Sessions')).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: 'Open bookmarked session Child' }),
    );
    expect(onResume).toHaveBeenCalledWith('child');
  });

  it('allows removing a bookmark directly from the top section', () => {
    const sessions: AgentSession[] = [
      createSession('bookmarked', 'Bookmarked Session', { isBookmarked: true }),
    ];
    const onToggleBookmark = vi.fn();

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        onToggleBookmark={onToggleBookmark}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Remove bookmark' }));
    expect(onToggleBookmark).toHaveBeenCalledWith('bookmarked');
  });

  it('shows overflow count when bookmarks exceed the featured shortcut cap', () => {
    const sessions: AgentSession[] = [
      createSession('s1', 'Session 1', {
        isBookmarked: true,
      }),
      createSession('s2', 'Session 2', { isBookmarked: true }),
      createSession('s3', 'Session 3', { isBookmarked: true }),
      createSession('s4', 'Session 4', { isBookmarked: true }),
      createSession('s5', 'Session 5', { isBookmarked: true }),
      createSession('s6', 'Session 6', { isBookmarked: true }),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    expect(screen.getByText('+1 more in history')).toBeInTheDocument();
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
        activeStatusFilter="all"
        searchQuery="beta"
      />,
    );

    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-child-beta')).toBeInTheDocument();
  });
});

import { fireEvent, render, screen } from '@testing-library/react';
import { SessionHistoryPanel } from '../SessionHistoryPanel';
import type { AgentSession } from '@/models/agent';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

if (!HTMLElement.prototype.hasPointerCapture) {
  HTMLElement.prototype.hasPointerCapture = () => false;
}

if (!HTMLElement.prototype.setPointerCapture) {
  HTMLElement.prototype.setPointerCapture = () => {};
}

if (!HTMLElement.prototype.releasePointerCapture) {
  HTMLElement.prototype.releasePointerCapture = () => {};
}

let animationFrameQueue: FrameRequestCallback[] = [];

global.requestAnimationFrame = ((callback: FrameRequestCallback) => {
  animationFrameQueue.push(callback);
  return animationFrameQueue.length;
}) as typeof global.requestAnimationFrame;

global.cancelAnimationFrame = ((id: number) => {
  const index = id - 1;
  if (index >= 0 && index < animationFrameQueue.length) {
    animationFrameQueue[index] = () => {};
  }
}) as typeof global.cancelAnimationFrame;

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

function flushAnimationFrames() {
  const callbacks = [...animationFrameQueue];
  animationFrameQueue = [];
  callbacks.forEach((callback) => callback(performance.now()));
}

function mockElementRect(
  element: Element,
  rect: Partial<DOMRect> & Pick<DOMRect, 'bottom'>,
) {
  Object.defineProperty(element, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({
      x: 0,
      y: 0,
      top: rect.top ?? 0,
      right: rect.right ?? 0,
      bottom: rect.bottom,
      left: rect.left ?? 0,
      width: rect.width ?? 0,
      height: rect.height ?? 0,
      toJSON: () => ({}),
    }),
  });
}

function getRenderedSessionIds(): string[] {
  return screen
    .getAllByTestId(/^session-card-/)
    .map((element) => element.getAttribute('data-testid')?.replace('session-card-', '') ?? '');
}

describe('SessionHistoryPanel', () => {
  beforeEach(() => {
    animationFrameQueue = [];
  });

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

  it('applies row-level indentation for expanded child sessions', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('child', 'Child', { parentSessionId: 'root' }),
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

    const rootRow = screen.getByTestId('session-card-root').closest('[role="listitem"]');
    const childRow = screen.getByTestId('session-card-child').closest('[role="listitem"]');

    expect(rootRow).not.toHaveStyle({ paddingLeft: '18px' });
    expect(childRow).toHaveStyle({ paddingLeft: '18px' });
    expect(childRow?.firstElementChild).toHaveClass('border-l', 'pl-3');
  });

  it('sorts root sessions by latest activity descending by default', () => {
    const sessions: AgentSession[] = [
      createSession('older', 'Older Session', {
        updatedAt: new Date('2026-03-18T00:00:00Z'),
      }),
      createSession('newest', 'Newest Session', {
        updatedAt: new Date('2026-03-21T00:00:00Z'),
      }),
      createSession('middle', 'Middle Session', {
        updatedAt: new Date('2026-03-20T00:00:00Z'),
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

    expect(getRenderedSessionIds()).toEqual(['newest', 'middle', 'older']);
  });

  it('prioritizes lastMessageAt over updatedAt for latest activity sorting', () => {
    const sessions: AgentSession[] = [
      createSession('metadata-only', 'Metadata Only', {
        updatedAt: new Date('2026-03-21T00:00:00Z'),
      }),
      createSession('recent-message', 'Recent Message', {
        updatedAt: new Date('2026-03-20T00:00:00Z'),
        lastMessageAt: new Date('2026-03-22T00:00:00Z'),
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

    expect(getRenderedSessionIds()).toEqual(['recent-message', 'metadata-only']);
  });

  it('toggles the session sort direction', () => {
    const sessions: AgentSession[] = [
      createSession('older', 'Older Session', {
        updatedAt: new Date('2026-03-18T00:00:00Z'),
      }),
      createSession('newest', 'Newest Session', {
        updatedAt: new Date('2026-03-21T00:00:00Z'),
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

    fireEvent.click(screen.getByRole('button', { name: 'Sort ascending' }));

    expect(getRenderedSessionIds()).toEqual(['older', 'newest']);
    expect(
      screen.getByRole('button', { name: 'Sort descending' }),
    ).toBeInTheDocument();
  });

  it('sorts sessions by name when the sort key changes', () => {
    const sessions: AgentSession[] = [
      createSession('zebra', 'Zebra'),
      createSession('apple', 'Apple'),
      createSession('mango', 'Mango'),
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
        initialSortKey="name"
        initialSortDirection="asc"
      />,
    );

    expect(getRenderedSessionIds()).toEqual(['apple', 'mango', 'zebra']);
  });

  it('renders the session list with list semantics', () => {
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

  it('loads more sessions when the load more button is pressed', () => {
    const onLoadMore = vi.fn();

    render(
      <SessionHistoryPanel
        sessions={[createSession('root', 'Root')]}
        isLoading={false}
        {...defaultProps}
        hasMoreSessions
        onLoadMore={onLoadMore}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load more' }));

    expect(onLoadMore).toHaveBeenCalledTimes(1);
  });

  it('does not load more sessions while a page is already loading', () => {
    const onLoadMore = vi.fn();

    render(
      <SessionHistoryPanel
        sessions={[createSession('root', 'Root')]}
        isLoading={false}
        {...defaultProps}
        hasMoreSessions
        isLoadingMoreSessions
        onLoadMore={onLoadMore}
        activeStatusFilter="all"
        searchQuery=""
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Loading more...' }));

    expect(onLoadMore).not.toHaveBeenCalled();
  });

  it('loads more sessions when the sentinel approaches the nearest scrollable parent', () => {
    const onLoadMore = vi.fn();
    const { container } = render(
      <div data-testid="scroll-parent" style={{ overflowY: 'auto' }}>
        <SessionHistoryPanel
          sessions={[createSession('root', 'Root')]}
          isLoading={false}
          {...defaultProps}
          hasMoreSessions
          onLoadMore={onLoadMore}
          activeStatusFilter="all"
          searchQuery=""
        />
      </div>,
    );

    const scrollParent = container.firstElementChild;
    expect(scrollParent).not.toBeNull();

    const sentinel = screen.getByTestId('session-history-load-more-sentinel');
    mockElementRect(scrollParent!, { bottom: 800, top: 0, height: 800 });
    mockElementRect(sentinel, { bottom: 1200, top: 1199, height: 1 });

    flushAnimationFrames();
    expect(onLoadMore).not.toHaveBeenCalled();

    mockElementRect(sentinel, { bottom: 900, top: 899, height: 1 });
    fireEvent.scroll(scrollParent!);
    flushAnimationFrames();

    expect(onLoadMore).toHaveBeenCalledTimes(1);
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
      createSession('s7', 'Session 7', { isBookmarked: true }),
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

    expect(screen.getByText('+1 more bookmarked sessions')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Browse all bookmarked sessions' }),
    ).toBeInTheDocument();
  });

  it('sorts bookmarked preview sessions by latest message activity', () => {
    const sessions: AgentSession[] = [
      createSession('metadata-bookmark', 'Metadata Bookmark', {
        isBookmarked: true,
        updatedAt: new Date('2026-03-21T00:00:00Z'),
      }),
      createSession('message-bookmark', 'Message Bookmark', {
        isBookmarked: true,
        updatedAt: new Date('2026-03-20T00:00:00Z'),
        lastMessageAt: new Date('2026-03-22T00:00:00Z'),
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

    const bookmarkedButtons = screen.getAllByRole('button', {
      name: /Open bookmarked session /,
    });

    expect(bookmarkedButtons[0]).toHaveAccessibleName(
      'Open bookmarked session Message Bookmark',
    );
    expect(bookmarkedButtons[1]).toHaveAccessibleName(
      'Open bookmarked session Metadata Bookmark',
    );
  });

  it('can focus the main history list on bookmarked sessions', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('bookmarked-child', 'Bookmarked Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
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

    fireEvent.click(screen.getByRole('button', { name: 'Bookmarked (1)' }));

    expect(screen.getByText('Showing bookmarked sessions')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-root')).toBeInTheDocument();
    expect(
      screen.getByTestId('session-card-bookmarked-child'),
    ).toBeInTheDocument();
    expect(screen.queryByTestId('session-card-other-root')).not.toBeInTheDocument();
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

  it('supports controlled showBookmarkedOnly prop', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('bookmarked-child', 'Bookmarked Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];

    const { rerender } = render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
        showBookmarkedOnly={true}
      />,
    );

    expect(screen.getByText('Showing bookmarked sessions')).toBeInTheDocument();

    rerender(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
        showBookmarkedOnly={false}
      />,
    );

    expect(screen.queryByText('Showing bookmarked sessions')).not.toBeInTheDocument();
  });

  it('triggers onShowBookmarkedOnlyChange callback when clicked', () => {
    const sessions: AgentSession[] = [
      createSession('root', 'Root'),
      createSession('bookmarked-child', 'Bookmarked Child', {
        parentSessionId: 'root',
        isBookmarked: true,
      }),
    ];
    const onShowBookmarkedOnlyChange = vi.fn();

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        {...defaultProps}
        activeStatusFilter="all"
        searchQuery=""
        showBookmarkedOnly={false}
        onShowBookmarkedOnlyChange={onShowBookmarkedOnlyChange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Bookmarked (1)' }));

    expect(onShowBookmarkedOnlyChange).toHaveBeenCalledWith(true);
  });
});

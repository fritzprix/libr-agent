import { render, screen } from '@testing-library/react';
import { SessionHistoryPanel } from '../SessionHistoryPanel';
import type { AgentSession } from '@/models/agent';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';

// Mock react-i18next to avoid missing i18n initialization in test setup
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultString: string) => defaultString || key,
  }),
}));

// Mock SessionCard to expose descendantCount
vi.mock('../SessionCard', () => ({
  SessionCard: ({ session, descendantCount }: { session: AgentSession; descendantCount: number }) => (
    <div data-testid={`session-card-${session.id}`} data-descendant-count={descendantCount}>
      {session.name}
    </div>
  ),
}));

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('SessionHistoryPanel descendant counts', () => {
  it('correctly calculates descendant counts for a session tree', () => {
    const sessions: AgentSession[] = [
      { id: 'root', name: 'Root', status: 'idle', model: 'test-model', provider: 'test-provider', createdAt: new Date(), updatedAt: new Date(), parentSessionId: undefined },
      { id: 'child1', name: 'Child 1', status: 'idle', model: 'test-model', provider: 'test-provider', createdAt: new Date(), updatedAt: new Date(), parentSessionId: 'root' },
      { id: 'child2', name: 'Child 2', status: 'idle', model: 'test-model', provider: 'test-provider', createdAt: new Date(), updatedAt: new Date(), parentSessionId: 'root' },
      { id: 'grandchild1', name: 'Grandchild 1', status: 'idle', model: 'test-model', provider: 'test-provider', createdAt: new Date(), updatedAt: new Date(), parentSessionId: 'child2' },
    ];

    render(
      <SessionHistoryPanel
        sessions={sessions}
        isLoading={false}
        activeTab="all"
        searchQuery=""
        onActiveTabChange={() => {}}
        onSearchQueryChange={() => {}}
        onRefresh={() => {}}
        onResume={() => {}}
        onDelete={() => {}}
        onDeleteOnly={() => {}}
      />
    );

    // Root should have 3 descendants (child1, child2, grandchild1)
    expect(screen.getByTestId('session-card-root')).toHaveAttribute('data-descendant-count', '3');

    // Child 1 should have 0 descendants
    expect(screen.getByTestId('session-card-child1')).toHaveAttribute('data-descendant-count', '0');

    // Child 2 should have 1 descendant (grandchild1)
    expect(screen.getByTestId('session-card-child2')).toHaveAttribute('data-descendant-count', '1');

    // Grandchild 1 should have 0 descendants
    expect(screen.getByTestId('session-card-grandchild1')).toHaveAttribute('data-descendant-count', '0');
  });
});

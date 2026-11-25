import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionPluginsOverview } from '@/types/skills';
import { SessionPluginsSection } from '../SessionPluginsSection';

const mockState = vi.hoisted(() => ({
  overview: undefined as SessionPluginsOverview | undefined,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { time?: string }) =>
      options?.time ? `${key}:${options.time}` : key,
  }),
}));

vi.mock('@/features/agent/hooks/useSessionPlugins', () => ({
  useSessionPlugins: () => ({
    overview: mockState.overview,
    isLoading: false,
    refresh: vi.fn(),
  }),
}));

describe('SessionPluginsSection', () => {
  beforeEach(() => {
    mockState.overview = undefined;
  });

  it('shows the empty hint when no plugin is active', () => {
    mockState.overview = { active: [], history: [] };
    render(<SessionPluginsSection sessionId="s1" />);
    expect(
      screen.getByText('agent.toolsModal.plugins.empty'),
    ).toBeInTheDocument();
  });

  it('lists active plugins with scope, hooks and skills, plus history', () => {
    mockState.overview = {
      active: [
        {
          id: 'plugin-aaaa',
          sourceUrl: 'https://github.com/o/session-plugin',
          scope: 'session',
          installedAt: 1767323045,
          skillNames: ['alpha', 'beta'],
          hookEvents: ['SessionStart'],
          hooksApproved: true,
          enabled: true,
        },
        {
          id: 'plugin-bbbb',
          sourceUrl: 'https://github.com/o/global-plugin',
          scope: 'global',
          installedAt: 1767225600,
          skillNames: [],
          hookEvents: ['SessionStart'],
          hooksApproved: false,
          enabled: false,
        },
      ],
      history: [
        {
          sourceId: 'plugin-cccc',
          sourceUrl: 'https://github.com/o/old',
          action: 'uninstall',
          at: 1767398400,
        },
      ],
    };
    render(<SessionPluginsSection sessionId="s1" />);

    expect(
      screen.getByText('https://github.com/o/session-plugin'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('https://github.com/o/global-plugin'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('agent.toolsModal.plugins.scopeSession'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('agent.toolsModal.plugins.scopeGlobal'),
    ).toBeInTheDocument();
    expect(screen.getByText(/alpha, beta/)).toBeInTheDocument();
    expect(
      screen.getByText(/agent\.toolsModal\.plugins\.hooksNotApproved/),
    ).toBeInTheDocument();
    expect(
      screen.getByText('agent.toolsModal.plugins.actionUninstall'),
    ).toBeInTheDocument();
  });
});

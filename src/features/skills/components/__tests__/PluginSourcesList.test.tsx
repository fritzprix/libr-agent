import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';

import type { PluginSourceOverview } from '@/types/skills';
import { PluginSourcesList } from '../PluginSourcesList';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

function source(
  overrides: Partial<PluginSourceOverview> = {},
): PluginSourceOverview {
  return {
    id: 'plugin-aabbccddeeff0011',
    sourceUrl: 'https://github.com/example/plugin',
    skillNames: ['demo'],
    hookEvents: ['SessionStart', 'SessionEnd', 'PostToolUseFailure'],
    hooksApproved: true,
    ...overrides,
  };
}

describe('PluginSourcesList hook status markers', () => {
  it('shows executed marker for supported events and not-supported marker for unsupported events', () => {
    render(
      <PluginSourcesList
        sources={[source()]}
        isBusy={false}
        removingId={null}
        onUpdate={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    // First action button expands the source row (chevron).
    fireEvent.click(screen.getAllByRole('button')[0]);

    expect(screen.getByText('SessionStart')).toBeInTheDocument();
    expect(screen.getByText('SessionEnd')).toBeInTheDocument();
    expect(screen.getAllByText(/Executed/)).toHaveLength(2);
    expect(screen.getByText('PostToolUseFailure')).toBeInTheDocument();
    expect(screen.getByText(/Not supported/)).toBeInTheDocument();
  });
});

// #2921: manifest metadata from `.claude-plugin/plugin.json` must be shown
// instead of the raw source id.
describe('PluginSourcesList plugin manifest display (#2921)', () => {
  it('shows the manifest name, version, and description', () => {
    render(
      <PluginSourcesList
        sources={[
          source({
            pluginName: 'superpowers',
            pluginVersion: '1.2.3',
            pluginDescription: 'Skills for agents',
          }),
        ]}
        isBusy={false}
        removingId={null}
        onUpdate={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    expect(screen.getByText('superpowers')).toBeInTheDocument();
    expect(screen.getByText('v1.2.3')).toBeInTheDocument();
    expect(screen.getByText('Skills for agents')).toBeInTheDocument();
    expect(
      screen.queryByText('plugin-aabbccddeeff0011'),
    ).not.toBeInTheDocument();
  });

  it('falls back to the source id when the plugin has no manifest', () => {
    render(
      <PluginSourcesList
        sources={[source()]}
        isBusy={false}
        removingId={null}
        onUpdate={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    expect(screen.getByText('plugin-aabbccddeeff0011')).toBeInTheDocument();
  });
});

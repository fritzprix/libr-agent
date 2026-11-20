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

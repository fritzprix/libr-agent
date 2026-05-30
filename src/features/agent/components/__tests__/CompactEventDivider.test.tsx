import '@testing-library/jest-dom';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { CompactEventDivider } from '../shared/CompactEventDivider';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      _key: string,
      fallbackOrOptions?: string | { defaultValue?: string; count?: number },
    ) => {
      if (typeof fallbackOrOptions === 'string') {
        return fallbackOrOptions;
      }

      return fallbackOrOptions?.defaultValue ?? _key;
    },
  }),
}));

describe('CompactEventDivider', () => {
  it('renders compacted range previews without exposing internal ids', () => {
    render(
      <CompactEventDivider
        latestIncludedPreview="The explicit cache manager is in gemini/service.ts."
        condensedCount={7}
      />,
    );

    expect(screen.getByText('Context compacted')).toBeInTheDocument();
    expect(
      screen.getByText(
        /The explicit cache manager is in gemini\/service\.ts\./,
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('7 messages condensed')).toBeInTheDocument();
  });

  it('reveals the saved summary when expanded', () => {
    render(
      <CompactEventDivider
        latestIncludedPreview="Latest context"
        summary="Summary body goes here."
      />,
    );

    expect(screen.queryByText('Summary body goes here.')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Context compacted/i }));

    expect(screen.getByText('Summary body goes here.')).toBeInTheDocument();
    expect(screen.getByText('Summary')).toBeInTheDocument();
  });
});

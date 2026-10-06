import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Spotlight } from '../types';
import { HubHintCard } from '../components/HubHintCard';
import { ReleaseSpotlightCard } from '../components/ReleaseSpotlightCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; version?: string }) => {
      if (key === 'spotlight.releaseTitle' && options?.version) {
        return `What's new in v${options.version}`;
      }
      return options?.defaultValue || key;
    },
  }),
}));

const tip: Spotlight = {
  id: 'starter-tasks',
  sinceVersion: '0.9.10',
  titleKey: 'spotlight.items.starterTasks.title',
  bodyKey: 'spotlight.items.starterTasks.body',
  ctaLabelKey: 'spotlight.items.starterTasks.cta',
  href: { type: 'route', path: '/scheduled-tasks' },
  surfaces: ['release', 'hub'],
  priority: 1,
};

describe('ReleaseSpotlightCard', () => {
  it('renders items and invokes CTA / dismiss handlers', () => {
    const onCta = vi.fn();
    const onDismissAll = vi.fn();

    render(
      <ReleaseSpotlightCard
        appVersion="0.9.21"
        items={[tip]}
        onCta={onCta}
        onDismissAll={onDismissAll}
      />,
    );

    expect(screen.getByTestId('release-spotlight-card')).toBeInTheDocument();
    expect(
      screen.getByText("What's new in v0.9.21"),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', {
        name: 'spotlight.items.starterTasks.cta',
      }),
    );
    expect(onCta).toHaveBeenCalledWith(tip);

    fireEvent.click(screen.getAllByRole('button', { name: 'Dismiss' })[0]!);
    expect(onDismissAll).toHaveBeenCalled();
  });

  it('renders nothing when items are empty', () => {
    const { container } = render(
      <ReleaseSpotlightCard
        appVersion="0.9.21"
        items={[]}
        onCta={vi.fn()}
        onDismissAll={vi.fn()}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});

describe('HubHintCard', () => {
  it('renders tip and invokes CTA / next / hide', () => {
    const onCta = vi.fn();
    const onNext = vi.fn();
    const onHide = vi.fn();

    render(
      <HubHintCard tip={tip} onCta={onCta} onNext={onNext} onHide={onHide} />,
    );

    expect(screen.getByTestId('hub-hint-card')).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', {
        name: 'spotlight.items.starterTasks.cta',
      }),
    );
    expect(onCta).toHaveBeenCalledWith(tip);

    fireEvent.click(screen.getByRole('button', { name: 'Next tip' }));
    expect(onNext).toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Hide' }));
    expect(onHide).toHaveBeenCalled();
  });
});

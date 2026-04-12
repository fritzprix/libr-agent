import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';
import { DangerZoneSettings } from '../DangerZoneSettings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultString?: string) => defaultString || key,
  }),
}));

describe('DangerZoneSettings', () => {
  it('requires typing RESET before enabling factory reset', async () => {
    render(
      <DangerZoneSettings
        isDeleting={false}
        isResetting={false}
        onDelete={vi.fn().mockResolvedValue(undefined)}
        onReset={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'Reset All Data & Settings' }),
    );

    const resetButton = await screen.findByRole('button', {
      name: 'Reset Everything',
    });
    expect(resetButton).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText('Type RESET'), {
      target: { value: 'RESET' },
    });

    expect(resetButton).toBeEnabled();
  });
});

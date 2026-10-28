import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import { TEST_DISPLAY_SETTINGS } from '@/test/fixtures/settings';
import GeneralTab from '../GeneralTab';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

describe('GeneralTab', () => {
  it('rerenders when tool detail level changes', () => {
    const onChange = vi.fn();
    const onDisplaySettingsChange = vi.fn();

    const { rerender } = render(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={TEST_DISPLAY_SETTINGS}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(
      screen.getByLabelText('Tool Detail Level'),
    ).toHaveTextContent('Simple (tool name only)');

    rerender(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={{
          ...TEST_DISPLAY_SETTINGS,
          toolDetailLevel: 'developer',
        }}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(
      screen.getByLabelText('Tool Detail Level'),
    ).toHaveTextContent('Developer (params, errors, timing)');
  });

  it('rerenders when message layout changes', () => {
    const onChange = vi.fn();
    const onDisplaySettingsChange = vi.fn();

    const { rerender } = render(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={TEST_DISPLAY_SETTINGS}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(screen.getByLabelText('Message Layout')).toHaveTextContent(
      'Document (full-width coding layout)',
    );

    rerender(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={{
          ...TEST_DISPLAY_SETTINGS,
          messageLayout: 'bubble',
        }}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(screen.getByLabelText('Message Layout')).toHaveTextContent(
      'Bubble (classic chat bubbles)',
    );
  });

  it('rerenders when color theme changes and triggers change handler', () => {
    const onChange = vi.fn();
    const onDisplaySettingsChange = vi.fn();

    const { rerender } = render(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={TEST_DISPLAY_SETTINGS}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    const neutralRadio = screen.getByRole('radio', {
      name: /Neutral \(Monochrome\)/i,
    });
    expect(neutralRadio).toHaveAttribute('aria-checked', 'true');

    const amberRadio = screen.getByRole('radio', {
      name: /Libr Amber \(Warmth\)/i,
    });
    expect(amberRadio).toHaveAttribute('aria-checked', 'false');

    fireEvent.click(amberRadio);
    expect(onDisplaySettingsChange).toHaveBeenCalledWith('colorTheme', 'amber');

    rerender(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={{
          ...TEST_DISPLAY_SETTINGS,
          colorTheme: 'amber',
        }}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(amberRadio).toHaveAttribute('aria-checked', 'true');
    expect(neutralRadio).toHaveAttribute('aria-checked', 'false');
  });

  it('reports UI size choices and reset through onUiScaleChange', () => {
    const onUiScaleChange = vi.fn();

    render(
      <GeneralTab
        localLanguage="en"
        onChange={vi.fn()}
        localDisplay={{ ...TEST_DISPLAY_SETTINGS, uiScale: 1.25 }}
        onDisplaySettingsChange={vi.fn()}
        onUiScaleChange={onUiScaleChange}
      />,
    );

    expect(screen.getByRole('radio', { name: '125%' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    fireEvent.click(screen.getByRole('radio', { name: '150%' }));
    expect(onUiScaleChange).toHaveBeenCalledWith(1.5);
    fireEvent.click(screen.getByRole('button', { name: 'Reset to 100%' }));
    expect(onUiScaleChange).toHaveBeenCalledWith(1);
  });
});

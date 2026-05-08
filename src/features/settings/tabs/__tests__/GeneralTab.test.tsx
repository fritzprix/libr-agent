import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_SETTING } from '@/context/SettingsContext';
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
        localDisplay={DEFAULT_SETTING.display}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(screen.getByText('Simple (tool name only)')).toBeInTheDocument();

    rerender(
      <GeneralTab
        localLanguage="en"
        onChange={onChange}
        localDisplay={{
          ...DEFAULT_SETTING.display,
          toolDetailLevel: 'developer',
        }}
        onDisplaySettingsChange={onDisplaySettingsChange}
      />,
    );

    expect(
      screen.getByText('Developer (params, errors, timing)'),
    ).toBeInTheDocument();
  });
});

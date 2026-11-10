import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import en from '@/locales/en/common.json';
import { TEST_SYSTEM_SETTINGS } from '@/test/fixtures/settings';
import type { IsolationLevel } from '@/lib/services/settings-service';
import { AdvancedShellIsolationSection } from '../advanced/AdvancedShellIsolationSection';

// Resolve keys against the real English locale so a missing key fails here.
function lookup(key: string): string {
  const value = key
    .split('.')
    .reduce<unknown>(
      (node, part) =>
        node && typeof node === 'object'
          ? (node as Record<string, unknown>)[part]
          : undefined,
      en,
    );
  return typeof value === 'string' ? value : `missing:${key}`;
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => lookup(key) }),
}));

function renderSection(level: IsolationLevel) {
  const onChange = vi.fn();
  render(
    <AdvancedShellIsolationSection
      systemSettingsProps={{
        localSystemSettings: {
          ...TEST_SYSTEM_SETTINGS,
          shellIsolationLevel: level,
        },
        networkSettingsChanged: false,
        onChange,
      }}
    />,
  );
  return onChange;
}

describe('AdvancedShellIsolationSection', () => {
  it.each(['basic', 'medium', 'high'] as const)(
    'describes the selected %s level',
    (level) => {
      renderSection(level);
      const select = screen.getByLabelText('Shell Isolation Level');
      const help = document.getElementById(
        select.getAttribute('aria-describedby') ?? '',
      );
      expect(help).toHaveTextContent(
        lookup(`settings.advanced.shellIsolationHelp.${level}`),
      );
      expect(help?.textContent).not.toContain('missing:');
    },
  );

  it('states per-OS behavior and the fallback for high', () => {
    const help = lookup('settings.advanced.shellIsolationHelp.high');
    expect(help).toContain('unshare');
    expect(help).toContain('sandbox-exec');
    expect(help).toContain('Windows: same as Medium');
    expect(help).toContain('Medium is used');
  });

  it('reports the chosen level', () => {
    const onChange = renderSection('medium');
    fireEvent.change(screen.getByLabelText('Shell Isolation Level'), {
      target: { value: 'high' },
    });
    expect(onChange).toHaveBeenCalledWith('shellIsolationLevel', 'high');
  });
});

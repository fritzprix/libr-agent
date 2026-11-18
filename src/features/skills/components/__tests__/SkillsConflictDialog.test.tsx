import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';

import type { SkillImportPreview } from '@/types/skills';
import type { PendingInstall } from '../../skills-management-types';
import { SkillsConflictDialog } from '../SkillsConflictDialog';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

// Radix ScrollArea needs a ResizeObserver in jsdom.
global.ResizeObserver = class ResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
};

const LONG_HOOK_COMMAND =
  '"${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd" session-start';

function buildPendingInstall(
  preview: Partial<SkillImportPreview> = {},
): PendingInstall {
  return {
    kind: 'github',
    sourceValue: 'https://github.com/obra/superpowers',
    approveHooks: true,
    selectedOverwriteNames: [],
    preview: {
      kind: 'plugin',
      discoveredSkills: [{ name: 'brainstorming', description: 'Draft ideas' }],
      conflicts: [],
      hooks: [
        {
          event: 'SessionStart',
          command: LONG_HOOK_COMMAND,
          executionStatus: 'executed',
        },
      ],
      hooksRequireApproval: true,
      ...preview,
    },
  };
}

function renderDialog(pendingInstall: PendingInstall | null) {
  return render(
    <SkillsConflictDialog
      pendingInstall={pendingInstall}
      userConflicts={[]}
      systemConflicts={[]}
      isInstallingLocal={false}
      isInstallingGithub={false}
      onOpenChange={vi.fn()}
      onToggleOverwrite={vi.fn()}
      onApproveHooksChange={vi.fn()}
      onConfirm={vi.fn()}
      onConfirmWithoutHooks={vi.fn()}
    />,
  );
}

function getFooter(): HTMLElement {
  const footer = document.querySelector<HTMLElement>(
    '[data-slot="alert-dialog-footer"]',
  );
  if (!footer) {
    throw new Error('alert-dialog-footer not rendered');
  }
  return footer;
}

describe('SkillsConflictDialog footer layout (#2896)', () => {
  it('keeps every hook-install action inside a footer that can wrap', () => {
    renderDialog(buildPendingInstall());

    const footer = getFooter();
    // Narrow widths stack the buttons; wider widths wrap them instead of
    // clipping the last button at the dialog edge.
    expect(footer).toHaveClass('flex-col');
    expect(footer).toHaveClass('sm:flex-row');
    expect(footer).toHaveClass('sm:flex-wrap');
    expect(footer).toHaveClass('min-w-0');

    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install without hook approval' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install and approve hooks' }),
    ).toBeInTheDocument();
  });

  it('renders the short footer set when the plugin ships no hooks', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [],
        hooksRequireApproval: false,
        discoveredSkills: [],
      }),
    );

    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Install without hook approval' }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install remaining skills' }),
    ).toBeInTheDocument();
  });
});

describe('SkillsConflictDialog long hook command wrapping (#2896)', () => {
  it('wraps long hook commands and matchers instead of widening the dialog', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [
          {
            event: 'SessionStart',
            matcher: 'startup|clear|compact',
            command: LONG_HOOK_COMMAND,
            executionStatus: 'executed',
          },
        ],
      }),
    );

    const command = screen.getByText(LONG_HOOK_COMMAND);
    expect(command).toHaveClass('break-all');
    // The wrapping block must be allowed to shrink inside its flex/grid parents.
    expect(command.parentElement).toHaveClass('min-w-0');

    const matcher = screen.getByText('startup|clear|compact');
    expect(matcher).toHaveClass('break-all');
    expect(matcher).toHaveClass('min-w-0');
  });
});

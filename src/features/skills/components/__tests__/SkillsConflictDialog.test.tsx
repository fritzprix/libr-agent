import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';

import type { SkillImportPreview } from '@/types/skills';
import type { PendingInstall } from '../../skills-management-types';
import { SkillsConflictDialog } from '../SkillsConflictDialog';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      _key: string,
      fallbackOrOptions?: string | Record<string, unknown>,
      options?: Record<string, unknown>,
    ) => {
      if (typeof fallbackOrOptions === 'string') {
        if (!options) return fallbackOrOptions;
        return fallbackOrOptions.replace(/\{\{(\w+)\}\}/g, (_m, name: string) =>
          String(options[name] ?? ''),
        );
      }
      if (fallbackOrOptions && typeof fallbackOrOptions === 'object') {
        const { defaultValue, ...vars } = fallbackOrOptions as {
          defaultValue?: string;
        } & Record<string, unknown>;
        if (!defaultValue) return _key;
        return defaultValue.replace(/\{\{(\w+)\}\}/g, (_m, name: string) =>
          String(vars[name] ?? ''),
        );
      }
      return _key;
    },
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
    pluginOptions: {},
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
      onUserConfigChange={vi.fn()}
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
      screen.getByRole('button', { name: 'Install without approval' }),
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
      screen.queryByRole('button', { name: 'Install without approval' }),
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

  it('shows exec-form args so everything the hook runs is visible (#3005)', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [
          {
            event: 'PreToolUse',
            command: 'node',
            args: ['-e', 'process.exit(0)'],
            executionStatus: 'executed',
          },
        ],
      }),
    );

    // The dialog must render the whole argv, not just `node`.
    expect(screen.getByText('node -e process.exit(0)')).toBeInTheDocument();
  });
});

describe('SkillsConflictDialog http hook header disclosure (#3041 G1.1)', () => {
  it('shows declared header templates and allowed env-var names', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [
          {
            event: 'PreToolUse',
            matcher: 'workspace__runShell',
            command: 'http://127.0.0.1:9/hook',
            executionStatus: 'executed',
            httpHeaders: { Authorization: 'Bearer $API_KEY' },
            httpAllowedEnvVars: ['API_KEY', 'OTHER_VAR'],
          },
        ],
      }),
    );

    expect(screen.getByText(/Authorization: Bearer \$API_KEY/)).toBeInTheDocument();
    expect(screen.getByText(/API_KEY, OTHER_VAR/)).toBeInTheDocument();
  });

  it('omits the header and env-var lines for command hooks', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [
          {
            event: 'SessionStart',
            command: 'echo hi',
            executionStatus: 'executed',
          },
        ],
      }),
    );

    expect(screen.queryByText(/Headers:/)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/May read environment variables/),
    ).not.toBeInTheDocument();
  });
});

describe('SkillsConflictDialog PreToolUse tool-approval disclosure (#2925 D3)', () => {
  it('shows tool-approval warning when preview contains a PreToolUse hook', () => {
    renderDialog(
      buildPendingInstall({
        hooks: [
          {
            event: 'PreToolUse',
            matcher: 'workspace__runShell',
            command: 'echo check',
            executionStatus: 'executed',
          },
        ],
      }),
    );

    expect(
      screen.getByText(
        /approved plugin hooks and MCP servers may run during your sessions/i,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        /PreToolUse hooks that can allow, ask for confirmation on, or block tool calls/i,
      ),
    ).toBeInTheDocument();
  });

  it('omits tool-approval warning when preview has no PreToolUse hook', () => {
    renderDialog(buildPendingInstall());

    expect(
      screen.queryByText(/PreToolUse hooks that can allow/i),
    ).not.toBeInTheDocument();
  });
});

// #2921: the install preview must surface the plugin manifest summary.
describe('SkillsConflictDialog plugin manifest summary (#2921)', () => {
  it('renders the manifest name, version, and description', () => {
    renderDialog(
      buildPendingInstall({
        pluginManifest: {
          name: 'superpowers',
          version: '1.2.3',
          description: 'Skills for agents',
        },
      }),
    );

    expect(screen.getByText('superpowers')).toBeInTheDocument();
    expect(screen.getByText('v1.2.3')).toBeInTheDocument();
    expect(screen.getByText('Skills for agents')).toBeInTheDocument();
  });

  it('omits the manifest block when the plugin carries no manifest', () => {
    renderDialog(buildPendingInstall());

    expect(screen.queryByText('superpowers')).not.toBeInTheDocument();
  });
});

// #2924: plugin-declared MCP servers must be shown and approved at install.
describe('SkillsConflictDialog MCP server approval (#2924)', () => {
  const mcpPreview = {
    hooks: [],
    hooksRequireApproval: false,
    mcpServers: [
      {
        name: 'fetch',
        transport: 'stdio' as const,
        command: 'uvx',
        args: 'mcp-fetch --max 10',
        envNames: ['API_KEY', 'TOKEN'],
      },
      {
        name: 'remote',
        transport: 'http' as const,
        url: 'https://x/mcp',
        headerNames: ['Authorization'],
      },
    ],
    mcpRequireApproval: true,
  };

  it('shows each declared server with its full command line and URL', () => {
    renderDialog(buildPendingInstall(mcpPreview));

    expect(screen.getByText('Plugin MCP servers')).toBeInTheDocument();
    expect(screen.getByText('fetch')).toBeInTheDocument();
    expect(screen.getByText('uvx mcp-fetch --max 10')).toBeInTheDocument();
    expect(screen.getByText('remote')).toBeInTheDocument();
    expect(screen.getByText('https://x/mcp')).toBeInTheDocument();
  });

  // Gatekeeper G1.3: the disclosure shows declared env/header NAMES so the
  // one-time approval is informed (values are never carried to the UI).
  it('discloses declared env and header names (G1.3)', () => {
    renderDialog(buildPendingInstall(mcpPreview));

    expect(screen.getByText('env: API_KEY, TOKEN')).toBeInTheDocument();
    expect(screen.getByText('headers: Authorization')).toBeInTheDocument();
  });

  it('requires approval when only MCP servers are present (no hooks)', () => {
    renderDialog(buildPendingInstall(mcpPreview));

    expect(
      screen.getByText('Allow MCP servers to start'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install without approval' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Install and approve servers' }),
    ).toBeInTheDocument();
  });

  it('uses the combined hooks-and-servers label when both are present', () => {
    renderDialog(
      buildPendingInstall({
        mcpServers: [
          { name: 'fetch', transport: 'stdio', command: 'uvx', args: 'mcp-fetch' },
        ],
        mcpRequireApproval: true,
      }),
    );

    expect(
      screen.getByText('Allow hooks to run and MCP servers to start'),
    ).toBeInTheDocument();
    // The hooks footer label wins when hooks exist.
    expect(
      screen.getByRole('button', { name: 'Install and approve hooks' }),
    ).toBeInTheDocument();
  });

  it('omits the MCP block when the plugin declares no servers', () => {
    renderDialog(buildPendingInstall());

    expect(
      screen.queryByText('Plugin MCP servers'),
    ).not.toBeInTheDocument();
  });
});

// #2923 gatekeeper G1.2: the preview must disclose the LibrAgent services a
// subagent IS granted, not only the tools that are dropped as unmapped.
describe('SkillsConflictDialog plugin agent grant disclosure (#2923 G1.2)', () => {
  it('shows granted services and the workspace caveat for mapped tools', () => {
    renderDialog(
      buildPendingInstall({
        pluginAgents: [
          {
            name: 'reviewer',
            description: 'Reviews code',
            unmappedTools: ['CustomLint'],
            grantedServices: ['workspace'],
            grantsAllServices: false,
          },
        ],
      }),
    );

    expect(screen.getByText('Subagents to install')).toBeInTheDocument();
    expect(screen.getByText('reviewer')).toBeInTheDocument();
    expect(
      screen.getByText('Granted services: workspace'),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'The workspace service includes file writes and shell execution.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Tools with no mapping are not granted (restricted entries such as Bash(git:*) count as unmapped): CustomLint',
      ),
    ).toBeInTheDocument();
  });

  it('discloses the all-services grant when the definition has no tools line', () => {
    renderDialog(
      buildPendingInstall({
        pluginAgents: [
          {
            name: 'plain',
            description: '',
            unmappedTools: [],
            grantedServices: [],
            grantsAllServices: true,
          },
        ],
      }),
    );

    expect(
      screen.getByText(
        'No tools line: all LibrAgent services will be granted.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'The workspace service includes file writes and shell execution.',
      ),
    ).toBeInTheDocument();
  });

  it('states an explicit none grant when no declared tool maps', () => {
    renderDialog(
      buildPendingInstall({
        pluginAgents: [
          {
            name: 'custom',
            description: '',
            unmappedTools: ['MyTool'],
            grantedServices: [],
            grantsAllServices: false,
          },
        ],
      }),
    );

    expect(
      screen.getByText('No services granted (no tool maps).'),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(
        'The workspace service includes file writes and shell execution.',
      ),
    ).not.toBeInTheDocument();
  });
});

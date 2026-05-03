import '@testing-library/jest-dom';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { RecommendedPresets } from '../RecommendedPresets';
import type { MCPServerEntity } from '@/models/chat';
import type { MCPServerPreset } from '@/lib/backend/mcp-server-config';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultValueOrOptions:
        | string
        | { defaultValue?: string; name?: string }
        | undefined,
    ) => {
      if (typeof defaultValueOrOptions === 'string') {
        return defaultValueOrOptions;
      }

      const template = defaultValueOrOptions?.defaultValue ?? key;
      const name = defaultValueOrOptions?.name;

      return name ? template.replace('{{name}}', name) : template;
    },
  }),
}));

const basePreset: MCPServerPreset = {
  name: 'filesystem',
  description: 'Filesystem tools',
  transportType: 'stdio',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-filesystem'],
};

const installedServer: MCPServerEntity = {
  id: 'server-1',
  name: 'filesystem',
  isActive: true,
  transport: {
    type: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem'],
  },
  createdAt: new Date(),
  updatedAt: new Date(),
};

describe('RecommendedPresets', () => {
  it('marks a preset as installed when it exists in the full server list but not the paged grid', () => {
    render(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[]}
        allServers={[installedServer]}
        registryLoaded={true}
        onSetupPreset={vi.fn()}
      />,
    );

    expect(screen.getByText('Installed')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Install filesystem extension' }),
    ).not.toBeInTheDocument();
  });

  it('uses the visible installed grid as a temporary installed source before the full registry finishes loading', () => {
    render(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[installedServer]}
        allServers={[]}
        registryLoaded={false}
        onSetupPreset={vi.fn()}
      />,
    );

    expect(screen.getByText('Installed')).toBeInTheDocument();
  });

  it('blocks opening install flow until the full registry load completes', () => {
    const onSetupPreset = vi.fn();

    render(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[]}
        allServers={[]}
        registryLoaded={false}
        onSetupPreset={onSetupPreset}
      />,
    );

    const presetCard = screen.getByText('filesystem').closest('div[aria-disabled]');
    expect(screen.getByText('Loading')).toBeInTheDocument();
    expect(presetCard).toHaveAttribute('aria-disabled', 'true');

    if (!presetCard) {
      throw new Error('Expected preset card');
    }

    fireEvent.click(presetCard);
    expect(onSetupPreset).not.toHaveBeenCalled();
  });
});

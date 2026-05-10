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
  category: 'devtools',
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
        registryError={undefined}
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText('Installed')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Install filesystem extension' }),
    ).not.toBeInTheDocument();
  });

  it('keeps hook ordering stable when presets load after an empty render', () => {
    const { rerender } = render(
      <RecommendedPresets
        presets={[]}
        servers={[]}
        allServers={[]}
        registryLoaded={true}
        registryError={undefined}
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    rerender(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[]}
        allServers={[]}
        registryLoaded={true}
        registryError={undefined}
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(
      screen.getByRole('button', { name: 'Install filesystem extension' }),
    ).toBeInTheDocument();
  });

  it('uses the visible installed grid as a temporary installed source before the full registry finishes loading', () => {
    render(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[installedServer]}
        allServers={[]}
        registryLoaded={false}
        registryError={undefined}
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
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
        registryError={undefined}
        onSetupPreset={onSetupPreset}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
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

  it('shows the actual preset transport badge for installable presets', () => {
    render(
      <RecommendedPresets
        presets={[
          {
            ...basePreset,
            name: 'exa',
            transportType: 'sse',
            command: undefined,
            args: undefined,
            url: 'https://example.com/sse',
          },
        ]}
        servers={[]}
        allServers={[]}
        registryLoaded={true}
        registryError={undefined}
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText('sse')).toBeInTheDocument();
    expect(screen.queryByText('stdio')).not.toBeInTheDocument();
  });

  it('offers a retry action when the full registry load failed', () => {
    const onRetryRegistryLoad = vi.fn().mockResolvedValue(undefined);

    render(
      <RecommendedPresets
        presets={[basePreset]}
        servers={[]}
        allServers={[]}
        registryLoaded={false}
        registryError="Registry fetch failed"
        onSetupPreset={vi.fn()}
        onRetryRegistryLoad={onRetryRegistryLoad}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetryRegistryLoad).toHaveBeenCalledTimes(1);
  });
});

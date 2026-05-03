import '@testing-library/jest-dom';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { RecommendedPresets } from '../RecommendedPresets';
import type { MCPServerEntity } from '@/models/chat';
import type { MCPServerPreset } from '@/lib/backend/mcp-server-config';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultValue: string | { defaultValue?: string }) => {
      if (typeof defaultValue === 'string') {
        return defaultValue;
      }

      return defaultValue?.defaultValue ?? _key;
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
        allServers={[installedServer]}
        onSetupPreset={vi.fn()}
      />,
    );

    expect(screen.getByText('Installed')).toBeInTheDocument();
    expect(
      screen.queryByText('Install filesystem extension'),
    ).not.toBeInTheDocument();
  });
});

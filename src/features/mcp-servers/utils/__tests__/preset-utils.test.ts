import { describe, expect, it } from 'vitest';
import type { MCPServerEntity } from '@/models/chat';
import type { MCPServerPreset } from '@/lib/backend/mcp-server-config';
import {
  buildPresetMetadata,
  collectMeaningfulVariableDefaults,
  isMeaningfulPrefill,
  isPrefilledVariableDefinition,
  isRegistrySourcedServer,
  resolvePresetVariableRawValue,
} from '../preset-utils';

const baseServer = (
  overrides: Partial<MCPServerEntity> = {},
): MCPServerEntity => ({
  id: 'id-1',
  name: 'github',
  isActive: true,
  createdAt: new Date(0),
  updatedAt: new Date(0),
  transport: { type: 'http-sse', url: 'https://example.com' },
  ...overrides,
});

describe('isMeaningfulPrefill', () => {
  it('rejects empty and YOUR_* placeholders', () => {
    expect(isMeaningfulPrefill('')).toBe(false);
    expect(isMeaningfulPrefill('YOUR_API_KEY')).toBe(false);
    expect(isMeaningfulPrefill('Bearer YOUR_GITHUB_PAT')).toBe(false);
  });

  it('accepts real defaults', () => {
    expect(isMeaningfulPrefill('~/.cache/huggingface/hub')).toBe(true);
    expect(isMeaningfulPrefill('true')).toBe(true);
  });
});

describe('collectMeaningfulVariableDefaults', () => {
  it('keeps only meaningful env prefills regardless of required flag', () => {
    const preset: MCPServerPreset = {
      name: 'huggingface',
      category: 'ai',
      transportType: 'stdio',
      command: 'npx',
      env: {
        HF_TOKEN: '',
        HF_MODEL_CACHE_DIR: '~/.cache/huggingface/hub',
        HF_ALLOW_REMOTE_MODELS: 'true',
      },
      variableDefinitions: {
        HF_TOKEN: { required: true },
        HF_MODEL_CACHE_DIR: { required: false },
        HF_ALLOW_REMOTE_MODELS: { required: false },
      },
    };

    expect(collectMeaningfulVariableDefaults(preset)).toEqual({
      HF_MODEL_CACHE_DIR: '~/.cache/huggingface/hub',
      HF_ALLOW_REMOTE_MODELS: 'true',
    });
  });

  it('treats YOUR_* header/bearer templates as user input', () => {
    const preset: MCPServerPreset = {
      name: 'github',
      category: 'devtools',
      transportType: 'sse',
      url: 'https://api.githubcopilot.com/mcp/',
      env: {
        Authorization: 'Bearer YOUR_GITHUB_PAT',
      },
      variableDefinitions: {
        GITHUB_PAT: { required: true, target: 'bearer-token' },
      },
    };

    expect(resolvePresetVariableRawValue(preset, 'GITHUB_PAT', {
      target: 'bearer-token',
    })).toBe('YOUR_GITHUB_PAT');
    expect(collectMeaningfulVariableDefaults(preset)).toEqual({});
  });
});

describe('buildPresetMetadata', () => {
  it('stores variableDefaults for registry installs', () => {
    const preset: MCPServerPreset = {
      name: 'telegram',
      category: 'messaging',
      transportType: 'stdio',
      env: {
        TELEGRAM_MCP_SESSION: '~/.libragent/telegram_mcp',
        TELEGRAM_BOT_TOKEN: '',
      },
      variableDefinitions: {
        TELEGRAM_MCP_SESSION: { required: false },
        TELEGRAM_BOT_TOKEN: { required: true },
      },
    };

    const metadata = buildPresetMetadata(preset);
    expect(metadata?.source).toBe('registry');
    expect(metadata?.variableDefaults).toEqual({
      TELEGRAM_MCP_SESSION: '~/.libragent/telegram_mcp',
    });
  });
});

describe('isPrefilledVariableDefinition', () => {
  it('uses snapshotted variableDefaults so live edits do not move fields', () => {
    const metadata = {
      source: 'registry' as const,
      variableDefaults: {
        HF_MODEL_CACHE_DIR: '~/.cache/huggingface/hub',
      },
      variableDefinitions: {
        HF_TOKEN: { required: true },
        HF_MODEL_CACHE_DIR: { required: false },
      },
    };

    expect(
      isPrefilledVariableDefinition('HF_MODEL_CACHE_DIR', {}, metadata),
    ).toBe(true);
    expect(isPrefilledVariableDefinition('HF_TOKEN', {}, metadata)).toBe(false);
  });
});

describe('isRegistrySourcedServer', () => {
  it('returns true when metadata.source is registry', () => {
    expect(
      isRegistrySourcedServer(
        baseServer({ metadata: { source: 'registry' } }),
      ),
    ).toBe(true);
  });

  it('returns true when the name is in the registry preset set', () => {
    expect(
      isRegistrySourcedServer(
        baseServer({ metadata: undefined }),
        new Set(['github']),
      ),
    ).toBe(true);
  });

  it('returns false for custom servers', () => {
    expect(
      isRegistrySourcedServer(
        baseServer({
          name: 'my-custom',
          metadata: { source: 'custom' },
        }),
        new Set(['github']),
      ),
    ).toBe(false);
  });
});

import { describe, expect, it } from 'vitest';
import type { MCPServerPreset } from '@/lib/backend/mcp-server-config';
import { presetNeedsUserConfig } from '../preset-utils';
import {
  humanizeVerificationError,
  pendingVerificationHint,
} from '../verification-feedback';

describe('presetNeedsUserConfig', () => {
  it('returns false for zero-config stdio presets', () => {
    const preset: MCPServerPreset = {
      name: 'hn',
      category: 'search',
      transportType: 'stdio',
      command: 'npx',
      args: ['-y', '@fre4x/hn'],
    };
    expect(presetNeedsUserConfig(preset)).toBe(false);
  });

  it('returns true when a required API key has no meaningful default', () => {
    const preset: MCPServerPreset = {
      name: 'context7',
      category: 'devtools',
      transportType: 'sse',
      url: 'https://mcp.context7.com/mcp',
      env: { CONTEXT7_API_KEY: 'YOUR_API_KEY' },
      variableDefinitions: {
        CONTEXT7_API_KEY: {
          required: true,
          target: 'header',
        },
      },
    };
    expect(presetNeedsUserConfig(preset)).toBe(true);
  });

  it('returns false when every variable has a meaningful prefill', () => {
    const preset: MCPServerPreset = {
      name: 'prefilled',
      category: 'ai',
      transportType: 'stdio',
      command: 'npx',
      env: { CACHE_DIR: '~/.cache/x' },
      variableDefinitions: {
        CACHE_DIR: { target: 'env' },
      },
    };
    expect(presetNeedsUserConfig(preset)).toBe(false);
  });

  it('returns true for OAuth presets', () => {
    const preset: MCPServerPreset = {
      name: 'oauth-server',
      category: 'devtools',
      transportType: 'sse',
      url: 'https://example.com',
      authentication: {
        type: 'oauth2.1',
        clientId: 'id',
      },
    };
    expect(presetNeedsUserConfig(preset)).toBe(true);
  });
});

describe('humanizeVerificationError', () => {
  it('maps generic missing-binary probe errors to App Wizard guidance', () => {
    const result = humanizeVerificationError(
      "Failed to connect to 'hn': No such file or directory (os error 2)",
    );
    expect(result?.runtime).toBe('node');
    expect(result?.guidance).toMatch(/App Wizard/);
  });

  it('detects missing npx/node specifically', () => {
    const withNpx = humanizeVerificationError(
      "Failed to connect to 'hn': program not found: npx",
    );
    expect(withNpx?.runtime).toBe('node');
    expect(withNpx?.summary).toMatch(/Node\.js/i);
  });

  it('detects missing uvx', () => {
    const result = humanizeVerificationError(
      "Failed to connect to 'ddg-search': uvx: command not found",
    );
    expect(result?.runtime).toBe('uv');
  });

  it('detects timeouts', () => {
    const result = humanizeVerificationError(
      "Timed out connecting to 'serena' after 120s",
    );
    expect(result?.runtime).toBeNull();
    expect(result?.summary.toLowerCase()).toContain('timed out');
  });
});

describe('pendingVerificationHint', () => {
  it('escalates copy over time', () => {
    expect(pendingVerificationHint(2)).toMatch(/Starting/i);
    expect(pendingVerificationHint(15)).toMatch(/Downloading/i);
    expect(pendingVerificationHint(45)).toMatch(/Still working/i);
    expect(pendingVerificationHint(100)).toMatch(/longer/i);
  });
});

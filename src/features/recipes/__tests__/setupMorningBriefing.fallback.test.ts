import { describe, expect, it } from 'vitest';
import mcpServerConfig from '../../../../mcp-server.json';
import { FALLBACK_PRESETS } from '../setupMorningBriefing';

describe('Morning Briefing offline fallback presets', () => {
  it.each(['hn', 'yahoo-finance'])(
    'bundled mcp-server.json still has %s and the fallback is built from it',
    (name) => {
      const bundled = (
        mcpServerConfig as { mcpServers: Record<string, { args?: string[] }> }
      ).mcpServers[name];
      expect(bundled, `${name} must stay in mcp-server.json`).toBeDefined();

      const fallback = FALLBACK_PRESETS[name];
      expect(fallback).toBeDefined();
      expect(fallback.name).toBe(name);
      expect(fallback.transportType).toBe('stdio');
      expect(fallback.command).toBe('npx');
      expect(fallback.args).toEqual(bundled.args);
    },
  );
});

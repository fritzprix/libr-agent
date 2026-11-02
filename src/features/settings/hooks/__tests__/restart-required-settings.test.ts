import { describe, expect, it } from 'vitest';
import { TEST_SETTINGS } from '@/test/fixtures/settings';
import {
  networkSettingsChanged,
  restartRequiredSettingsChanged,
} from '../restart-required-settings';

const saved = TEST_SETTINGS;

describe('restartRequiredSettingsChanged', () => {
  it.each([
    ['system', 'httpServerPort', 12345],
    ['system', 'httpServerExpose', !saved.system.httpServerExpose],
    ['system', 'searchIndexFrequencyMinutes', 99],
    ['system', 'webActionTimeoutSeconds', 99],
    ['advanced', 'maxConcurrentActiveSessions', 99],
    ['advanced', 'maxSuspendedSessions', 99],
    ['advanced', 'maxConcurrentActiveProcesses', 99],
    ['advanced', 'maxSuspendedProcesses', 99],
  ] as const)('is true when %s.%s changes', (section, key, value) => {
    const current = {
      ...saved,
      [section]: { ...saved[section], [key]: value },
    };
    expect(restartRequiredSettingsChanged(current, saved)).toBe(true);
  });

  it('is false for unchanged settings and live-applied settings', () => {
    expect(restartRequiredSettingsChanged(saved, saved)).toBe(false);
    const current = {
      ...saved,
      uiLanguage: 'ko',
      system: { ...saved.system, mcpToolTimeoutSeconds: 99 },
      advanced: { ...saved.advanced, diffContextLines: 99 },
    };
    expect(restartRequiredSettingsChanged(current, saved)).toBe(false);
  });
});

describe('networkSettingsChanged', () => {
  it('only tracks the HTTP server port and expose flag', () => {
    const port = { ...saved, system: { ...saved.system, httpServerPort: 1 } };
    const index = {
      ...saved,
      system: { ...saved.system, searchIndexFrequencyMinutes: 99 },
    };
    expect(networkSettingsChanged(port, saved)).toBe(true);
    expect(networkSettingsChanged(index, saved)).toBe(false);
  });
});

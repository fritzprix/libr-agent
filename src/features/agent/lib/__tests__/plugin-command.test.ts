import { describe, expect, it } from 'vitest';
import { parsePluginInstallCommand } from '../plugin-command';

describe('parsePluginInstallCommand', () => {
  it('extracts the git url and preserves its casing', () => {
    expect(
      parsePluginInstallCommand('/plugin install https://github.com/Owner/Repo'),
    ).toBe('https://github.com/Owner/Repo');
  });

  it('is case-insensitive on the command and tolerant of whitespace', () => {
    expect(parsePluginInstallCommand('  /PLUGIN   Install   owner/repo  ')).toBe(
      'owner/repo',
    );
  });

  it.each([
    '/plugin install',
    '/plugin list',
    '/plugin uninstall owner/repo',
    '/plugin install a b',
    'plugin install owner/repo',
  ])('returns null for %s', (text) => {
    expect(parsePluginInstallCommand(text)).toBeNull();
  });
});

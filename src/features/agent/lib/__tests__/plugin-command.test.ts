import { describe, expect, it } from 'vitest';
import {
  parsePluginInstallCommand,
  parsePluginUpdateCommand,
} from '../plugin-command';

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

// #2927 item 4: /plugin update <name|git-url>
describe('parsePluginUpdateCommand', () => {
  it('extracts the target and preserves its casing', () => {
    expect(
      parsePluginUpdateCommand('/plugin update https://github.com/Owner/Repo'),
    ).toBe('https://github.com/Owner/Repo');
    expect(parsePluginUpdateCommand('/plugin update superpowers')).toBe(
      'superpowers',
    );
  });

  it('is case-insensitive and tolerant of whitespace', () => {
    expect(parsePluginUpdateCommand('  /PLUGIN   UPDATE   owner/repo  ')).toBe(
      'owner/repo',
    );
  });

  it.each([
    '/plugin update',
    '/plugin update a b',
    '/plugin install owner/repo',
    '/plugin list',
    'plugin update owner/repo',
  ])('returns null for %s', (text) => {
    expect(parsePluginUpdateCommand(text)).toBeNull();
  });
});

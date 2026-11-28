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

  it('extracts a local folder path (#2928)', () => {
    expect(parsePluginInstallCommand('/plugin install /home/u/my-plugin')).toBe(
      '/home/u/my-plugin',
    );
    expect(parsePluginInstallCommand('/plugin install ./plugins/my-plugin')).toBe(
      './plugins/my-plugin',
    );
  });

  it('accepts a quoted path with spaces (#2928)', () => {
    expect(
      parsePluginInstallCommand('/plugin install "/home/u/my plugins/x"'),
    ).toBe('/home/u/my plugins/x');
  });

  it('extracts a non-GitHub git URL with a ref (#2928)', () => {
    expect(
      parsePluginInstallCommand('  /plugin install https://git.example.com/o/r.git?ref=v1'),
    ).toBe('https://git.example.com/o/r.git?ref=v1');
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

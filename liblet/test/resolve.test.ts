import * as path from 'node:path';

import { describe, expect, it } from 'vitest';

import { findContinueSession, normalizeWorkspace } from '../src/resolve.js';
import type { SessionMetadata } from '../src/types.js';

const session = (
  id: string,
  workspaceOverride: string | null,
): SessionMetadata => ({
  id,
  status: 'idle',
  model: 'm',
  provider: 'p',
  createdAt: 0,
  updatedAt: 0,
  executionMode: 'normal',
  workspaceOverride,
});

describe('normalizeWorkspace', () => {
  it('lower-cases the whole path on win32 only', () => {
    const input = path.resolve('Mixed/Case/Dir');
    expect(normalizeWorkspace(input, 'win32')).toBe(input.toLowerCase());
    expect(normalizeWorkspace(input, 'linux')).toBe(input);
  });

  it('strips trailing separators', () => {
    const bare = path.resolve('dir');
    expect(normalizeWorkspace(bare + path.sep, 'linux')).toBe(bare);
  });

  it('resolves a relative path against the cwd', () => {
    expect(normalizeWorkspace('.', 'linux')).toBe(
      path.resolve(process.cwd()).replace(/[\\/]+$/, ''),
    );
  });
});

describe('findContinueSession', () => {
  const cwd = path.resolve('Mixed/Case/Project');
  const sessions = [
    session('elsewhere', path.resolve('elsewhere')),
    session('match', cwd.toUpperCase() + path.sep),
    session('no-workspace', null),
  ];

  it('matches case-insensitively on win32 despite case and trailing separator', () => {
    expect(findContinueSession(sessions, cwd, 'win32')?.id).toBe('match');
  });

  it('is case-sensitive on posix', () => {
    expect(findContinueSession(sessions, cwd, 'linux')).toBeUndefined();
    expect(
      findContinueSession(sessions, cwd, 'linux') ??
        findContinueSession([session('exact', cwd)], cwd, 'linux')?.id,
    ).toBe('exact');
  });

  it('skips sessions without a workspace and returns undefined with no match', () => {
    expect(
      findContinueSession([session('no-workspace', null)], cwd, 'win32'),
    ).toBeUndefined();
    expect(
      findContinueSession(
        [session('elsewhere', path.resolve('elsewhere'))],
        cwd,
      ),
    ).toBeUndefined();
  });
});

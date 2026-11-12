import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const { listLocalBranches } = require('../cleanup-merged-branches.cjs') as {
  listLocalBranches: (opts?: { mergedInto?: string; cwd?: string }) => string[];
};

describe('cleanup-merged-branches listLocalBranches', () => {
  let repo: string;

  const git = (...args: string[]) =>
    execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim();

  const commit = (file: string) => {
    writeFileSync(join(repo, file), file);
    git('add', file);
    git('commit', '-q', '-m', file);
  };

  beforeEach(() => {
    repo = mkdtempSync(join(tmpdir(), 'cleanup-merged-branches-'));
    git('init', '-q', '-b', 'main');
    git('config', 'user.email', 'test@example.invalid');
    git('config', 'user.name', 'Test');
    git('config', 'commit.gpgsign', 'false');
    commit('base.txt');
    git('branch', 'dev/0.9.x');
    // Merged: its tip is the base tip.
    git('branch', 'feat/merged');
    // Not merged: one commit ahead of main.
    git('switch', '-q', '-c', 'feat/unmerged');
    commit('extra.txt');
  });

  afterEach(() => {
    rmSync(repo, { recursive: true, force: true });
  });

  it('lists real branches only when HEAD is detached (#2707)', () => {
    git('switch', '-q', '--detach', 'main');
    // The line that used to reach `git merge-base` and crash the script.
    // Git prints it in the user's language, so match its shape, not its text.
    expect(git('branch', '--format=%(refname:short)')).toMatch(/^\(.+\)$/m);

    const branches = listLocalBranches({ cwd: repo });

    expect(branches.sort()).toEqual(['feat/merged', 'feat/unmerged']);
    for (const branch of branches) {
      // Every name must be a ref that `git merge-base` accepts.
      expect(() => git('rev-parse', '--verify', branch)).not.toThrow();
    }
  });

  it('leaves out main and dev/* and filters by --merged', () => {
    git('switch', '-q', 'main');

    expect(listLocalBranches({ cwd: repo }).sort()).toEqual([
      'feat/merged',
      'feat/unmerged',
    ]);
    expect(listLocalBranches({ mergedInto: 'main', cwd: repo })).toEqual([
      'feat/merged',
    ]);
  });
});

import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const { listLocalBranches } = require('../cleanup-merged-branches.cjs') as {
  listLocalBranches: (opts?: { mergedInto?: string; cwd?: string }) => string[];
};

const gitEnv: NodeJS.ProcessEnv = {
  ...process.env,
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_TERMINAL_PROMPT: '0',
  GIT_OPTIONAL_LOCKS: '0',
  GIT_PAGER: '',
  HUSKY: '0',
  GIT_AUTHOR_NAME: 'Test',
  GIT_AUTHOR_EMAIL: 'test@example.invalid',
  GIT_COMMITTER_NAME: 'Test',
  GIT_COMMITTER_EMAIL: 'test@example.invalid',
};

describe('cleanup-merged-branches listLocalBranches', () => {
  let repo: string;

  const git = (...args: string[]) =>
    execFileSync('git', args, {
      cwd: repo,
      encoding: 'utf8',
      env: gitEnv,
      timeout: 15000,
      windowsHide: true,
    }).trim();

  const commit = (file: string) => {
    writeFileSync(join(repo, file), file);
    git('add', file);
    git('commit', '-q', '--no-gpg-sign', '-m', file);
  };

  // Windows git init/commit is slow under Defender; pay setup once and give
  // the hook more than the 10s default. Two tests used to each rebuild the repo.
  beforeAll(() => {
    repo = mkdtempSync(join(tmpdir(), 'cleanup-merged-branches-'));
    git('init', '-q', '-b', 'main');
    const hooksDir = join(repo, '.git', 'hooks-disabled');
    mkdirSync(hooksDir);
    git('config', 'core.hooksPath', hooksDir);
    git('config', 'core.fsmonitor', 'false');
    commit('base.txt');
    git('branch', 'dev/0.9.x');
    // Merged: its tip is the base tip.
    git('branch', 'feat/merged');
    // Not merged: one commit ahead of main.
    git('switch', '-q', '-c', 'feat/unmerged');
    commit('extra.txt');
  }, 30000);

  afterAll(() => {
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

#!/usr/bin/env node
/**
 * Remove Git branches that are merged and have no open pull request.
 *
 * `main` and `dev/*` are long-lived and never deleted. A branch counts as
 * merged when either:
 *   - its tip is an ancestor of the base branch (normal merge), or
 *   - a merged pull request used that branch as its head (squash/rebase
 *     merge leaves the tip outside the base ancestry).
 *
 * Usage:
 *   node scripts/cleanup-merged-branches.cjs [--base dev/0.9.x] [--execute]
 *                                            [--remote] [--no-fetch]
 *
 * Default is a dry run: candidates are printed, nothing is deleted.
 * --execute deletes local branches. --remote additionally deletes the
 * matching remote-tracking branches on origin (requires write access).
 */

'use strict';

const { spawnSync } = require('child_process');

function parseArgs(argv) {
  const args = {
    base: 'dev/0.9.x',
    execute: false,
    remote: false,
    fetch: true,
  };
  for (const arg of argv) {
    if (arg === '--execute') args.execute = true;
    else if (arg === '--remote') args.remote = true;
    else if (arg === '--no-fetch') args.fetch = false;
    else if (arg.startsWith('--base=')) args.base = arg.slice('--base='.length);
    else {
      console.error(`Unknown argument: ${arg}`);
      process.exit(1);
    }
  }
  return args;
}

function run(cmd, cmdArgs, { allowFail = false, cwd } = {}) {
  const res = spawnSync(cmd, cmdArgs, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  if (res.error) {
    console.error(`Failed to run ${cmd}: ${res.error.message}`);
    process.exit(1);
  }
  if (res.status !== 0 && !allowFail) {
    console.error(
      `${cmd} ${cmdArgs.join(' ')} failed: ${(res.stderr || '').trim()}`,
    );
    process.exit(1);
  }
  return {
    status: res.status,
    stdout: (res.stdout || '').trim(),
    stderr: (res.stderr || '').trim(),
  };
}

const git = (cmdArgs, opts) => run('git', cmdArgs, opts).stdout;
const gitRes = (cmdArgs, opts) => run('git', cmdArgs, opts);
const ghJson = (cmdArgs) => JSON.parse(run('gh', cmdArgs).stdout);

function isLongLived(branch) {
  return branch === 'main' || branch === 'master' || branch.startsWith('dev/');
}

/**
 * Local branch names that are not long-lived, optionally only those merged
 * into `mergedInto`.
 *
 * Reads `refs/heads` with `git for-each-ref`, which lists real branches only.
 * `git branch` also prints a `(HEAD detached at <sha>)` placeholder when HEAD
 * is detached; that line is not a ref and made `git merge-base` fail (#2707).
 */
function listLocalBranches({ mergedInto, cwd } = {}) {
  const cmdArgs = ['for-each-ref', '--format=%(refname:short)'];
  if (mergedInto) cmdArgs.push(`--merged=${mergedInto}`);
  cmdArgs.push('refs/heads');
  return git(cmdArgs, { cwd })
    .split('\n')
    .map((line) => line.trim())
    .filter((b) => b && !isLongLived(b));
}

function isAncestor(ref, baseRef) {
  const res = gitRes(['merge-base', '--is-ancestor', ref, baseRef], {
    allowFail: true,
  });
  if (res.status === 0) return true;
  if (res.status === 1) return false;
  console.error(`git merge-base failed for ${ref}: ${res.stderr}`);
  process.exit(1);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const baseRef = `origin/${args.base}`;
  git(['rev-parse', '--verify', baseRef]);

  if (args.fetch) git(['fetch', 'origin', '--prune']);

  // Pull request state is authoritative: squash/rebase merges break ancestry.
  const openHeads = new Set(
    ghJson([
      'pr',
      'list',
      '--state',
      'open',
      '--limit',
      '500',
      '--json',
      'headRefName',
    ]).map((pr) => pr.headRefName),
  );
  const mergedHeads = new Set(
    ghJson([
      'pr',
      'list',
      '--state',
      'merged',
      '--limit',
      '1000',
      '--json',
      'headRefName',
    ]).map((pr) => pr.headRefName),
  );

  const localBranches = listLocalBranches();

  const remoteBranches = git(['ls-remote', '--heads', 'origin'])
    .split('\n')
    .map((line) => (line.split('\t')[1] || '').replace(/^refs\/heads\//, ''))
    .filter((b) => b && !isLongLived(b));

  const mergedLocal = localBranches.filter(
    (b) => !openHeads.has(b) && (isAncestor(b, baseRef) || mergedHeads.has(b)),
  );
  const mergedRemote = remoteBranches.filter((b) => {
    if (openHeads.has(b)) return false;
    const originRef = `origin/${b}`;
    const hasOriginRef =
      gitRes(['rev-parse', '--verify', originRef], { allowFail: true })
        .status === 0;
    return (
      (hasOriginRef && isAncestor(originRef, baseRef)) || mergedHeads.has(b)
    );
  });

  console.log(`Base: ${baseRef}`);
  console.log(`Local merged branches (${mergedLocal.length}):`);
  for (const b of mergedLocal) console.log(`  ${b}`);
  if (args.remote) {
    console.log(`Remote merged branches (${mergedRemote.length}):`);
    for (const b of mergedRemote) console.log(`  origin/${b}`);
  }

  if (!args.execute) {
    console.log('Dry run. Pass --execute to delete.');
    return;
  }

  const skipped = [];
  for (const b of mergedLocal) {
    const res = gitRes(['branch', '-D', b], { allowFail: true });
    if (res.status !== 0) skipped.push(`${b}: ${res.stderr}`);
  }

  if (args.remote && mergedRemote.length > 0) {
    const CHUNK = 20;
    for (let i = 0; i < mergedRemote.length; i += CHUNK) {
      const chunk = mergedRemote.slice(i, i + CHUNK);
      const res = gitRes(['push', 'origin', '--delete', ...chunk], {
        allowFail: true,
      });
      if (res.status !== 0)
        skipped.push(`origin ${chunk.join(', ')}: ${res.stderr}`);
    }
  }

  const remaining = listLocalBranches({ mergedInto: baseRef });

  console.log(
    `Deleted. Remaining merged non-long-lived local branches: ${remaining.length}`,
  );
  for (const b of remaining) console.log(`  ${b}`);
  if (skipped.length > 0) {
    console.log('Skipped:');
    for (const s of skipped) console.log(`  ${s}`);
  }
}

if (require.main === module) {
  main();
}

module.exports = { isLongLived, listLocalBranches };

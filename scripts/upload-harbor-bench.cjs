#!/usr/bin/env node
/**
 * Harbor job uploader script.
 * Auto-detects the latest job in `jobs/` if no job path is specified.
 * Sets PYTHONUTF8=1 to prevent CP949 encoding errors on Windows.
 *
 * Usage:
 *   pnpm bench:upload                        # Uploads latest job in jobs/
 *   pnpm bench:upload jobs/2026-08-16__14-25-18 # Uploads specific job
 *   pnpm bench:upload --public              # Uploads latest job with --public flag
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { normalizeHermesJob } = require('./normalize-harbor-agent-metadata.cjs');

const repoRoot = path.resolve(__dirname, '..');
const jobsDir = path.join(repoRoot, 'jobs');
const isWindows = process.platform === 'win32';

/**
 * Check if `harbor` CLI binary exists in PATH.
 * @returns {boolean}
 */
function hasHarborCli() {
  const checkCmd = isWindows ? 'where' : 'which';
  const res = spawnSync(checkCmd, ['harbor'], {
    stdio: 'ignore',
    shell: isWindows,
  });
  return !res.error && res.status === 0;
}

/**
 * Resolve available python command (python3 or python).
 * @returns {string}
 */
function resolvePythonCmd() {
  const candidates = isWindows
    ? ['python', 'python3', 'py']
    : ['python3', 'python'];

  for (const cmd of candidates) {
    const res = spawnSync(cmd, ['--version'], {
      stdio: 'ignore',
      shell: isWindows,
    });
    if (!res.error && res.status === 0) {
      return cmd;
    }
  }
  return isWindows ? 'python' : 'python3';
}

const rawArgs = process.argv.slice(2);
let targetJob = null;
const passthroughFlags = [];

for (let i = 0; i < rawArgs.length; i += 1) {
  const arg = rawArgs[i];
  if (arg.startsWith('-')) {
    passthroughFlags.push(arg);
  } else if (!targetJob) {
    targetJob = arg;
  } else {
    passthroughFlags.push(arg);
  }
}

if (!targetJob) {
  if (!fs.existsSync(jobsDir)) {
    console.error(`Error: Jobs directory not found at ${jobsDir}`);
    process.exit(1);
  }

  const entries = fs.readdirSync(jobsDir, { withFileTypes: true });
  const jobDirs = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const fullPath = path.join(jobsDir, entry.name);
      const stat = fs.statSync(fullPath);
      return { name: entry.name, fullPath, mtime: stat.mtimeMs };
    })
    .sort((a, b) => b.mtime - a.mtime);

  if (jobDirs.length === 0) {
    console.error(`Error: No job directories found in ${jobsDir}`);
    process.exit(1);
  }

  targetJob = path.join('jobs', jobDirs[0].name);
  console.log(`[bench:upload] Auto-detected latest job: ${targetJob}`);
} else {
  if (
    !targetJob.startsWith('jobs') &&
    !path.isAbsolute(targetJob) &&
    fs.existsSync(path.join(jobsDir, targetJob))
  ) {
    targetJob = path.join('jobs', targetJob);
  }
}

const resolvedPath = path.isAbsolute(targetJob)
  ? targetJob
  : path.join(repoRoot, targetJob);

if (!fs.existsSync(resolvedPath)) {
  console.error(`Error: Specified job path does not exist: ${resolvedPath}`);
  process.exit(1);
}

const normalizedFiles = normalizeHermesJob(resolvedPath);
if (normalizedFiles > 0) {
  console.log(
    `[bench:upload] Normalized ${normalizedFiles} Hermes metadata file(s) ` +
      'to a stable release version.',
  );
}

console.log(`[bench:upload] Uploading ${targetJob}...`);

const env = {
  ...process.env,
  PYTHONUTF8: '1',
  PYTHONIOENCODING: 'utf-8',
};

const uploadArgs = ['upload', targetJob, ...passthroughFlags];

let res;
if (hasHarborCli()) {
  res = spawnSync('harbor', uploadArgs, {
    cwd: repoRoot,
    stdio: 'inherit',
    env,
    shell: isWindows,
  });
} else {
  const pythonCmd = resolvePythonCmd();
  console.log(
    `[bench:upload] 'harbor' CLI not found in PATH. Falling back to '${pythonCmd} -m harbor'...`,
  );
  res = spawnSync(pythonCmd, ['-m', 'harbor', ...uploadArgs], {
    cwd: repoRoot,
    stdio: 'inherit',
    env,
    shell: isWindows,
  });
}

if (res.error) {
  console.error(`[bench:upload] Execution error: ${res.error.message}`);
}

process.exit(res.status ?? (res.error ? 1 : 0));

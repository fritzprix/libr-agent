#!/usr/bin/env node
/**
 * Cross-platform Harbor / Terminal-Bench runner.
 * Dispatches to run-harbor-bench.ps1 on Windows and run-harbor-bench.sh elsewhere.
 *
 * Usage (same flags as the bash runner):
 *   node scripts/run-harbor-bench.cjs --preset terminal-bench --n-tasks 1
 */
'use strict';

const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..');
const isWindows = process.platform === 'win32';

/** @type {Record<string, string>} */
const FLAG_TO_PS = {
  '--preset': '-Preset',
  '--dataset': '-Dataset',
  '--path': '-Path',
  '--include': '-Include',
  '--n-tasks': '-NTasks',
  '--n-attempts': '-NAttempts',
  '--concurrent': '-Concurrent',
  '--api-url': '-ApiUrl',
  '--assistant-id': '-AssistantId',
  '--model': '-Model',
  '--execution-mode': '-ExecutionMode',
  '--timeout-multiplier': '-TimeoutMultiplier',
  '--agent-timeout-multiplier': '-AgentTimeoutMultiplier',
  '--verifier-env': '-VerifierEnv',
  '--ve': '-VerifierEnv',
};

/** @type {Record<string, string>} */
const SWITCH_TO_PS = {
  '--skip-health-check': '-SkipHealthCheck',
  '--dry-run': '-DryRun',
  '--debug': '-DebugHarbor',
};

/**
 * @param {string[]} argv
 * @returns {string[]}
 */
function toPowerShellArgs(argv) {
  /** @type {string[]} */
  const out = [];
  /** @type {Record<string, string[]>} */
  const accum = {};

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '-h' || arg === '--help') {
      out.push('-?');
      continue;
    }
    if (Object.prototype.hasOwnProperty.call(SWITCH_TO_PS, arg)) {
      out.push(SWITCH_TO_PS[arg]);
      continue;
    }
    if (Object.prototype.hasOwnProperty.call(FLAG_TO_PS, arg)) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('-')) {
        console.error(`Missing value for ${arg}`);
        process.exit(1);
      }
      const psFlag = FLAG_TO_PS[arg];
      if (!accum[psFlag]) {
        accum[psFlag] = [];
      }
      accum[psFlag].push(value);
      i += 1;
      continue;
    }
    console.error(`Unknown arg: ${arg}`);
    process.exit(1);
  }

  for (const [flag, vals] of Object.entries(accum)) {
    if (flag === '-VerifierEnv') {
      out.push(flag, vals.join(','));
    } else {
      out.push(flag, vals[vals.length - 1]);
    }
  }

  return out;
}

/**
 * @param {string} command
 * @param {string[]} args
 */
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: isWindows,
    env: process.env,
  });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  process.exit(result.status ?? 1);
}

const argv = process.argv.slice(2);

if (isWindows) {
  const ps1 = path.join(repoRoot, 'scripts', 'run-harbor-bench.ps1');
  const psArgs = toPowerShellArgs(argv);
  run('powershell', [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    ps1,
    ...psArgs,
  ]);
} else {
  const sh = path.join(repoRoot, 'scripts', 'run-harbor-bench.sh');
  run('bash', [sh, ...argv]);
}

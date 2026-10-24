#!/usr/bin/env node
/**
 * Cross-platform Harbor / Terminal-Bench runner.
 * Dispatches to run-harbor-bench.ps1 on Windows and run-harbor-bench.sh elsewhere.
 *
 * Usage (same flags as the bash runner):
 *   node scripts/run-harbor-bench.cjs --preset terminal-bench --n-tasks 1
 *
 * --isolated: launch the seeded bench instance (benchmarks/harbor/isolated_instance.py),
 * default --api-url/--assistant-id from its seed, relaunch it from the seed before every
 * trial (requires --concurrent 1), and stop it when the run ends.
 */
'use strict';

const fs = require('node:fs');
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
  '--agent': '-Agent',
  '--base-url': '-OpenAIBaseUrl',
  '--openai-base-url': '-OpenAIBaseUrl',
  '--api-key': '-OpenAIApiKey',
  '--openai-api-key': '-OpenAIApiKey',
  '--env-file': '-EnvFile',
  '--config-file': '-ConfigFile',
  '--bench-config': '-ConfigFile',
  '--timeout-multiplier': '-TimeoutMultiplier',
  '--agent-timeout-multiplier': '-AgentTimeoutMultiplier',
  '--smoke-timeout': '-SmokeTimeout',
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
    } else if (flag === '-Include') {
      for (const v of vals) {
        out.push(flag, v);
      }
    } else {
      out.push(flag, vals[vals.length - 1]);
    }
  }

  return out;
}

/**
 * @param {string} command
 * @param {string[]} args
 * @returns {number}
 */
function runStatus(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: isWindows,
    env: process.env,
  });

  if (result.error) {
    console.error(result.error.message);
    return 1;
  }

  return result.status ?? 1;
}

const ISOLATED_FLAG = '--isolated';
const INSTANCE_MODULE = 'benchmarks.harbor.isolated_instance';
const INSTANCE_ROOT = path.join(
  repoRoot,
  '.libragent',
  'work',
  'bench-instance',
);
const SEED_INFO_PATH = path.join(INSTANCE_ROOT, 'seed', 'seed.json');
const RELEASE_APP_BIN = path.join(
  repoRoot,
  'src-tauri',
  'target',
  'release',
  'libragent',
);

/**
 * @returns {{ assistantId: string, port: number }}
 */
function readSeedInfo() {
  if (!fs.existsSync(SEED_INFO_PATH)) {
    console.error(
      `No bench seed at ${SEED_INFO_PATH}.\n` +
        'Create it once: pnpm bench:instance:seed --assistant-id <uuid>',
    );
    process.exit(1);
  }
  const info = JSON.parse(fs.readFileSync(SEED_INFO_PATH, 'utf8'));
  if (typeof info.assistant_id !== 'string' || typeof info.port !== 'number') {
    console.error(`Invalid seed info: ${SEED_INFO_PATH}`);
    process.exit(1);
  }
  return { assistantId: info.assistant_id, port: info.port };
}

/**
 * @param {string[]} benchCommand
 * @param {string[]} benchArgs
 * @returns {number}
 */
function runIsolated(benchCommand, benchArgs) {
  if (process.platform !== 'linux') {
    // Stopping detached sidecars relies on /proc/<pid>/environ.
    console.error(`${ISOLATED_FLAG} is only supported on Linux.`);
    return 1;
  }
  const concurrentIndex = benchArgs.indexOf('--concurrent');
  if (concurrentIndex !== -1 && benchArgs[concurrentIndex + 1] !== '1') {
    console.error(
      `${ISOLATED_FLAG} restarts the bench instance before every trial and requires --concurrent 1.`,
    );
    return 1;
  }
  const { assistantId, port } = readSeedInfo();
  const args = [...benchArgs];
  if (!args.includes('--api-url')) {
    args.push('--api-url', `http://localhost:${port}/api`);
  }
  if (!args.includes('--assistant-id')) {
    args.push('--assistant-id', assistantId);
  }

  const launchStatus = runStatus('python', [
    '-m',
    INSTANCE_MODULE,
    'launch',
    '--app-bin',
    RELEASE_APP_BIN,
  ]);
  if (launchStatus !== 0) {
    return launchStatus;
  }

  // The Harbor adapter relaunches the instance from the seed before each trial.
  process.env.LIBRAGENT_BENCH_INSTANCE_APP_BIN = RELEASE_APP_BIN;
  process.env.LIBRAGENT_BENCH_INSTANCE_ROOT = INSTANCE_ROOT;
  try {
    return runStatus(benchCommand[0], [...benchCommand.slice(1), ...args]);
  } finally {
    runStatus('python', ['-m', INSTANCE_MODULE, 'stop']);
  }
}

const WORKSPACE_MODE_FLAG = '--workspace-mode';
const WORKSPACE_MODES = ['attach', 'managed', 'host'];

const rawArgv = process.argv.slice(2);
const isolated = rawArgv.includes(ISOLATED_FLAG);
const argv = rawArgv.filter((arg) => arg !== ISOLATED_FLAG);

/**
 * @param {string} mode
 */
function applyWorkspaceMode(mode) {
  process.env.LIBRAGENT_HARBOR_WORKSPACE_MODE = mode;
  // Read by the bench instance launcher and the Harbor adapter.
  if (mode === 'host') {
    process.env.LIBRAGENT_BENCH_INSTANCE_SANDBOX = '1';
  } else {
    delete process.env.LIBRAGENT_BENCH_INSTANCE_SANDBOX;
  }
}

/** @type {string[]} */
let workspaceModes = [];
const workspaceModeIndex = argv.indexOf(WORKSPACE_MODE_FLAG);
if (workspaceModeIndex !== -1) {
  const [, mode] = argv.splice(workspaceModeIndex, 2);
  if (mode !== 'all' && !WORKSPACE_MODES.includes(mode)) {
    const got = mode === undefined ? 'no value' : `'${mode}'`;
    console.error(
      `${WORKSPACE_MODE_FLAG} must be one of ${[...WORKSPACE_MODES, 'all'].join(', ')} (got ${got}).`,
    );
    process.exit(1);
  }
  if ((mode === 'host' || mode === 'all') && !isolated) {
    console.error(
      `${WORKSPACE_MODE_FLAG} ${mode} runs task commands on this machine and requires ${ISOLATED_FLAG}.`,
    );
    process.exit(1);
  }
  workspaceModes = mode === 'all' ? WORKSPACE_MODES : [mode];
  if (!isolated) {
    applyWorkspaceMode(mode);
  }
}

if (isolated) {
  const sh = path.join(repoRoot, 'scripts', 'run-harbor-bench.sh');
  let interrupted = false;
  // Keep the runner alive on Ctrl+C so the instance is always stopped.
  process.on('SIGINT', () => {
    interrupted = true;
  });
  let exitStatus = 0;
  for (const mode of workspaceModes.length > 0 ? workspaceModes : [null]) {
    if (mode !== null) {
      applyWorkspaceMode(mode);
      console.log(`==> Workspace mode: ${mode}`);
    }
    const status = runIsolated(['bash', sh], argv);
    if (status !== 0) {
      exitStatus = status;
    }
    if (interrupted) {
      break;
    }
  }
  process.exit(exitStatus);
} else if (isWindows) {
  const ps1 = path.join(repoRoot, 'scripts', 'run-harbor-bench.ps1');
  const psArgs = toPowerShellArgs(argv);
  process.exit(
    runStatus('powershell', [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      ps1,
      ...psArgs,
    ]),
  );
} else {
  const sh = path.join(repoRoot, 'scripts', 'run-harbor-bench.sh');
  process.exit(runStatus('bash', [sh, ...argv]));
}

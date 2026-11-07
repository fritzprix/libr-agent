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
 *
 * Ctrl+C in --isolated mode: the first press lets Harbor cancel the trial and tear down
 * its task containers (partial results are kept); the second press force-kills Harbor,
 * the bench instance and the task containers. Either way the instance is stopped, no
 * task container from this run is left, and the next --workspace-mode does not start.
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const { spawn, spawnSync } = require('node:child_process');
const {
  createInterruptController,
  killProcessTree,
  leftoverTaskContainers,
} = require('./lib/bench-interrupt.cjs');
const { planWorkspaceModes } = require('./lib/bench-workspace-mode.cjs');

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
// Launch the debug bench binary instead of the release one. Debug builds are
// the only ones that contain the debug-only E2E control-plane routes, so the
// session-e2e matrix requires this flag.
const DEBUG_APP_BIN_FLAG = '--debug-app-bin';
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
const DEBUG_APP_BIN = path.join(
  repoRoot,
  'src-tauri',
  'target',
  'debug',
  'libragent',
);
// Must match build.devUrl in src-tauri/tauri.conf.json.
const DEV_URL_PORT = 1420;

/** @param {number} port */
function portInUse(port) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host: '127.0.0.1', port });
    socket.setTimeout(500);
    socket.once('connect', () => {
      socket.destroy();
      resolve(true);
    });
    socket.once('timeout', () => {
      socket.destroy();
      resolve(false);
    });
    socket.once('error', () => resolve(false));
  });
}

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

// One SIGINT listener for the whole run; runBenchChild attaches the press handler.
const interrupt = createInterruptController();

/**
 * @param {string[]} args
 * @returns {string[]}
 */
function dockerLines(args) {
  const result = spawnSync('docker', args, { encoding: 'utf8' });
  if (result.error || result.status !== 0) return [];
  return result.stdout.split('\n').filter(Boolean);
}

function snapshotContainers() {
  return dockerLines(['ps', '-q']);
}

/** Kill task containers started during this run that Harbor did not tear down. */
function removeLeftoverContainers(/** @type {string[]} */ before) {
  const now = dockerLines([
    'ps',
    '--format',
    '{{.ID}}\t{{.Label "com.docker.compose.project"}}',
  ]).map((line) => {
    const [id, composeProject = ''] = line.split('\t');
    return { id, composeProject };
  });
  const leftovers = leftoverTaskContainers(before, now);
  if (leftovers.length > 0) {
    console.error(`Removing ${leftovers.length} leftover task container(s).`);
    spawnSync('docker', ['rm', '-f', ...leftovers], { stdio: 'ignore' });
  }
}

/**
 * Run the bench command, handling Ctrl+C: press 1 only reports (the terminal already
 * delivered SIGINT to Harbor, which cancels and cleans up); press 2 SIGKILLs the
 * whole process tree under the child.
 * @param {string} command
 * @param {string[]} args
 * @returns {Promise<number>}
 */
function runBenchChild(command, args) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      stdio: 'inherit',
      env: process.env,
    });
    interrupt.attach((press) => {
      if (press === 1) {
        console.error(
          '\nStopping: Harbor is cancelling the running trial and removing its task containers ' +
            '(partial results are kept). Press Ctrl+C again to force-stop immediately.',
        );
        return;
      }
      console.error(
        '\nForce-stopping: killing Harbor, the bench instance and task containers.',
      );
      if (child.pid !== undefined) killProcessTree(child.pid);
    });
    child.on('error', (error) => {
      console.error(error.message);
      interrupt.detach();
      resolve(1);
    });
    child.on('close', (code, signal) => {
      interrupt.detach();
      resolve(code ?? (signal ? 128 : 1));
    });
  });
}

/**
 * @param {string[]} benchCommand
 * @param {string[]} benchArgs
 * @returns {Promise<number>}
 */
/**
 * Serve the built frontend on the port `build.devUrl` points to
 * (src-tauri/tauri.conf.json). A debug binary loads its WebView from that URL
 * instead of the embedded dist/, so a headless bench run needs a server here
 * or the frontend never registers its LLM listener and sessions hang busy.
 *
 * @returns {Promise<number|null>} the server pid, or null when the port is
 *   already taken (e.g. a developer's `pnpm dev` is running).
 */
async function startFrontendServer() {
  if (await portInUse(DEV_URL_PORT)) {
    console.log(`==> Port ${DEV_URL_PORT} already in use; assuming it serves the frontend.`);
    return null;
  }
  const distDir = path.join(repoRoot, 'dist');
  if (!fs.existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(
      `No frontend build at ${distDir}. A debug bench binary loads its WebView from ` +
        `http://localhost:${DEV_URL_PORT}; run \`pnpm build\` first.`,
    );
  }
  const child = spawn(
    'python3',
    ['-m', 'http.server', String(DEV_URL_PORT), '--bind', '127.0.0.1', '-d', distDir],
    { stdio: 'ignore', detached: true },
  );
  child.unref();
  await new Promise((resolve) => setTimeout(resolve, 300));
  if (!(await portInUse(DEV_URL_PORT))) {
    killProcessTree(child.pid);
    throw new Error(`Frontend server on port ${DEV_URL_PORT} failed to start.`);
  }
  console.log(`==> Serving ${distDir} on http://localhost:${DEV_URL_PORT} (devUrl for the debug binary).`);
  return child.pid;
}

/** @param {number|null} pid */
function stopFrontendServer(pid) {
  if (pid !== null) killProcessTree(pid);
}

async function runIsolated(benchCommand, benchArgs) {
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
  const appBin = debugAppBin ? DEBUG_APP_BIN : RELEASE_APP_BIN;
  if (!fs.existsSync(appBin)) {
    console.error(
      `Bench instance binary not found: ${appBin}\n` +
        (debugAppBin
          ? 'Build the debug binary first: (cd src-tauri && cargo build)'
          : 'Build the release binary first: pnpm tauri build'),
    );
    return 1;
  }
  const args = [...benchArgs];
  if (!args.includes('--api-url')) {
    args.push('--api-url', `http://localhost:${port}/api`);
  }
  if (!args.includes('--assistant-id')) {
    args.push('--assistant-id', assistantId);
  }

  // Set before `launch` so instance_env() sees the flag on the FIRST start
  // too (it raises RUST_MIN_STACK for debug binaries carrying the E2E
  // control-plane filter chain, which overflows the default 2 MiB stack).
  if (debugAppBin) {
    // The E2E control plane routes only exist in debug builds and are opt-in;
    // the isolated instance forwards this flag into the app environment.
    process.env.LIBRAGENT_E2E_CONTROL = '1';
  }

  // A debug binary loads its frontend from build.devUrl (tauri.conf.json,
  // http://localhost:1420); the release binary embeds dist/ instead. Without
  // something serving that port the WebView stays blank, the frontend never
  // registers its `llm:completion-request` listener, and every session hangs
  // `busy` after "Emitted LLM completion request". Serve dist/ ourselves.
  let frontendServerPid = null;
  if (debugAppBin) {
    try {
      frontendServerPid = await startFrontendServer();
    } catch (err) {
      console.error(String(err && err.message ? err.message : err));
      return 1;
    }
  }

  const launchStatus = runStatus('python', [
    '-m',
    INSTANCE_MODULE,
    'launch',
    '--app-bin',
    appBin,
  ]);
  if (launchStatus !== 0) {
    return launchStatus;
  }

  // The Harbor adapter relaunches the instance from the seed before each trial.
  process.env.LIBRAGENT_BENCH_INSTANCE_APP_BIN = appBin;
  process.env.LIBRAGENT_BENCH_INSTANCE_ROOT = INSTANCE_ROOT;
  const containersBefore = snapshotContainers();
  try {
    return await runBenchChild(benchCommand[0], [
      ...benchCommand.slice(1),
      ...args,
    ]);
  } finally {
    const forced = interrupt.presses >= 2;
    runStatus('python', [
      '-m',
      INSTANCE_MODULE,
      'stop',
      ...(forced ? ['--force'] : []),
    ]);
    stopFrontendServer(frontendServerPid);
    if (interrupt.presses > 0) {
      removeLeftoverContainers(containersBefore);
    }
  }
}

const rawArgv = process.argv.slice(2);
const isolated = rawArgv.includes(ISOLATED_FLAG);
const debugAppBin = rawArgv.includes(DEBUG_APP_BIN_FLAG);

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

const plan = planWorkspaceModes(
  rawArgv.filter((arg) => arg !== ISOLATED_FLAG && arg !== DEBUG_APP_BIN_FLAG),
  isolated,
  process.env.LIBRAGENT_HARBOR_WORKSPACE_MODE,
);
if (debugAppBin && !isolated) {
  console.error(`${DEBUG_APP_BIN_FLAG} only applies to ${ISOLATED_FLAG} runs.`);
  process.exit(1);
}
if (plan.error !== null) {
  console.error(plan.error);
  process.exit(1);
}
const argv = plan.argv;
const workspaceModes = plan.modes;
if (plan.immediate !== null) {
  applyWorkspaceMode(plan.immediate);
}

if (isolated) {
  const sh = path.join(repoRoot, 'scripts', 'run-harbor-bench.sh');
  // Between runs (launch, stop) Ctrl+C must not kill the runner before the instance is
  // stopped; runBenchChild adds its own handler while Harbor is running.
  process.on('SIGINT', () => interrupt.press());
  (async () => {
    let exitStatus = 0;
    for (const mode of workspaceModes) {
      applyWorkspaceMode(mode);
      console.log(`==> Workspace mode: ${mode}`);
      const status = await runIsolated(['bash', sh], argv);
      if (status !== 0) {
        exitStatus = status;
      }
      if (interrupt.presses > 0) {
        break;
      }
    }
    process.exit(exitStatus);
  })();
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

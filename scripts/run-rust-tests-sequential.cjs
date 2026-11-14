#!/usr/bin/env node
/**
 * Run each Cargo integration test target one binary at a time.
 *
 * `cargo test --tests` compiles/links every `tests/*.rs` target in one graph.
 * LibrAgent test binaries are ~600MB+ (Tauri/WebKit). Linking even two at once
 * exhausts a 32GB machine. This runner never passes bare `--tests`.
 *
 * Usage:
 *   node scripts/run-rust-tests-sequential.cjs --profile ci-test
 *   node scripts/run-rust-tests-sequential.cjs --profile ci-test --test text_encoding_tests
 *   pnpm rust:test
 */

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const ROOT = path.join(__dirname, '..');
const TESTS_DIR = path.join(ROOT, 'src-tauri', 'tests');
const RUNNER = path.join(__dirname, 'run-rust-command.cjs');
// Workspace member crates whose unit tests run after the app's `--lib` step.
const WORKSPACE_TEST_CRATES = ['llm-client'];

function hasFlag(args, flagNames) {
  return args.some((arg) => {
    if (flagNames.includes(arg)) {
      return true;
    }
    return flagNames.some((flag) => arg.startsWith(`${flag}=`));
  });
}

function listIntegrationTestTargets() {
  return fs
    .readdirSync(TESTS_DIR)
    .filter((name) => name.endsWith('.rs'))
    .filter((name) => fs.statSync(path.join(TESTS_DIR, name)).isFile())
    .map((name) => name.slice(0, -3))
    .filter((name) => name !== 'common')
    .sort();
}

function shouldSkipTarget(name) {
  // Monolith harness is Linux/macOS-only (see tests/integration_tests.rs).
  if (process.platform === 'win32' && name === 'integration_tests') {
    return true;
  }
  return false;
}

function sleepMs(ms) {
  spawnSync(
    process.execPath,
    [
      '-e',
      `Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ${ms})`,
    ],
    {
      stdio: 'ignore',
    },
  );
}

/** Freshly linked ~30MB test exes are sometimes locked by Windows Defender (os error 5). */
function isTransientWindowsExecFailure(output) {
  return /never executed|os error 5|Access is denied|액세스가 거부/i.test(
    output,
  );
}

function runCargoTest(cargoArgs) {
  const env = { ...process.env };
  if (!env.CARGO_BUILD_JOBS) {
    env.CARGO_BUILD_JOBS = '1';
  }
  const result = spawnSync(process.execPath, [RUNNER, 'test', ...cargoArgs], {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: ['inherit', 'pipe', 'pipe'],
    env,
  });
  if (result.error) {
    console.error(result.error.message);
    return { status: 1, output: result.error.message };
  }
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  if (stdout) {
    process.stdout.write(stdout);
  }
  if (stderr) {
    process.stderr.write(stderr);
  }
  return {
    status: result.status ?? 1,
    output: `${stdout}${stderr}`,
  };
}

/** Windows only: Defender may lock a freshly linked test exe for a few seconds. */
const MAX_ATTEMPTS_WINDOWS = 5;
/** Linear backoff base: attempt N waits N * RETRY_BASE_DELAY_MS before retry N+1. */
const RETRY_BASE_DELAY_MS = 2000;

function maxAttemptsFor(platform) {
  return platform === 'win32' ? MAX_ATTEMPTS_WINDOWS : 1;
}

/**
 * Retry a transient Windows exec-lock failure with linear backoff.
 * `deps` exists for tests (inject `runCargoTest`/`sleepMs`/`maxAttempts`);
 * the CLI path always uses the defaults.
 */
function runCargoTestWithRetries(cargoArgs, targetLabel, deps = {}) {
  const run = deps.runCargoTest ?? runCargoTest;
  const sleep = deps.sleepMs ?? sleepMs;
  const maxAttempts = deps.maxAttempts ?? maxAttemptsFor(process.platform);
  let last = { status: 1, output: '' };
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    last = run(cargoArgs);
    if (last.status === 0) {
      return 0;
    }
    const canRetry =
      attempt < maxAttempts && isTransientWindowsExecFailure(last.output);
    if (!canRetry) {
      return last.status;
    }
    const delayMs = attempt * RETRY_BASE_DELAY_MS;
    console.error(
      `[rust-test-sequential] Transient Windows exec lock for ${targetLabel}; retry ${attempt}/${maxAttempts - 1} after ${delayMs / 1000}s...`,
    );
    sleep(delayMs);
  }
  return last.status;
}

function main() {
  const passthrough = process.argv.slice(2).filter((arg) => arg !== '--tests');

  // Single-target / explicit selection: do not fan out.
  if (
    hasFlag(passthrough, ['--test', '--bin', '--example', '--lib', '--doc'])
  ) {
    process.exit(runCargoTestWithRetries(passthrough, passthrough.join(' ')));
  }

  const targets = listIntegrationTestTargets().filter(
    (name) => !shouldSkipTarget(name),
  );

  if (targets.length === 0) {
    console.error('[rust-test-sequential] No integration test targets found.');
    process.exit(1);
  }

  // Workspace crates have none of the app's features, so a feature-specific
  // run (for example the edit-file one) leaves them out.
  const crates = hasFlag(passthrough, [
    '--features',
    '--no-default-features',
    '--all-features',
  ])
    ? []
    : WORKSPACE_TEST_CRATES;
  const leadSteps = 1 + crates.length;
  const totalSteps = targets.length + leadSteps;
  console.error(
    `[rust-test-sequential] ${totalSteps} targets (1 lib + ${crates.length} crate + ${targets.length} integration) — linking one binary at a time (avoids multi-target OOM)`,
  );

  console.error(`\n[rust-test-sequential] (1/${totalSteps}) --lib`);
  const libStatus = runCargoTestWithRetries(['--lib', ...passthrough], 'lib');
  if (libStatus !== 0) {
    console.error(`[rust-test-sequential] FAILED: lib (exit ${libStatus})`);
    process.exit(libStatus);
  }

  for (let i = 0; i < crates.length; i += 1) {
    const crate = crates[i];
    console.error(
      `\n[rust-test-sequential] (${i + 2}/${totalSteps}) -p ${crate} --lib`,
    );
    const status = runCargoTestWithRetries(
      ['-p', crate, '--lib', ...passthrough],
      crate,
    );
    if (status !== 0) {
      console.error(`[rust-test-sequential] FAILED: ${crate} (exit ${status})`);
      process.exit(status);
    }
  }

  for (let i = 0; i < targets.length; i += 1) {
    const target = targets[i];
    console.error(
      `\n[rust-test-sequential] (${i + leadSteps + 1}/${totalSteps}) --test ${target}`,
    );
    const status = runCargoTestWithRetries(
      ['--test', target, ...passthrough],
      target,
    );
    if (status !== 0) {
      console.error(
        `[rust-test-sequential] FAILED: ${target} (exit ${status})`,
      );
      console.error(
        `[rust-test-sequential] Stopping after first failure. Passed ${i + leadSteps}/${totalSteps} before fail.`,
      );
      process.exit(status);
    }
  }

  console.error(`\n[rust-test-sequential] All ${totalSteps} targets passed.`);
  process.exit(0);
}

if (require.main === module) {
  main();
}

module.exports = {
  isTransientWindowsExecFailure,
  maxAttemptsFor,
  runCargoTestWithRetries,
  MAX_ATTEMPTS_WINDOWS,
  RETRY_BASE_DELAY_MS,
};

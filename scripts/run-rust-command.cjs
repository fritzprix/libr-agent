#!/usr/bin/env node

const fs = require('node:fs');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const path = require('node:path');

const cargoArgs = process.argv.slice(2);

if (cargoArgs.length === 0) {
  console.error('Usage: node scripts/run-rust-command.cjs <cargo args...>');
  process.exit(1);
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function getCpuCount() {
  if (typeof os.availableParallelism === 'function') {
    return os.availableParallelism();
  }

  return os.cpus().length;
}

function getRecommendedBuildJobs() {
  // Match .cargo/config.toml [build] jobs = 2 hard cap intent for check/clippy/build.
  const cpuCount = Math.max(1, getCpuCount());
  const totalMemGiB = os.totalmem() / 1024 ** 3;
  const cpuLimitedJobs = clamp(cpuCount - 1, 1, 2);
  const memoryLimitedJobs = clamp(Math.floor(totalMemGiB / 8), 1, 2);

  return Math.min(cpuLimitedJobs, memoryLimitedJobs);
}

function getRecommendedTestBuildJobs() {
  // Integration binaries are ~600MB+ (Tauri). Concurrent rustc/link jobs thrash
  // a 32GB machine alongside the desktop IDE. Always serialize to 1.
  return 1;
}

function getDefaultBuildJobs(args) {
  const recommendedJobs =
    args[0] === 'test'
      ? getRecommendedTestBuildJobs()
      : getRecommendedBuildJobs();

  return recommendedJobs;
}

function getDefaultTestThreads() {
  return 1;
}

function hasFlag(args, flagNames) {
  return args.some((arg, index) => {
    if (flagNames.includes(arg)) {
      return true;
    }

    return flagNames.some(
      (flag) =>
        arg.startsWith(`${flag}=`) || (arg === flag && index < args.length - 1),
    );
  });
}

function commandExists(command) {
  const result = spawnSync('command', ['-v', command], {
    env: process.env,
    shell: true,
    stdio: 'ignore',
  });
  return result.status === 0;
}

const env = { ...process.env };
const isCargoTest = cargoArgs[0] === 'test';

// Broad `cargo test` / `cargo test --tests` without a single --test/--lib/…
// target links every tests/*.rs binary (~600MB+) in one graph → OOM on 32GB.
// Delegate to the sequential runner (never bare --tests in one cargo invocation).
if (
  isCargoTest &&
  !hasFlag(cargoArgs, ['--test', '--bin', '--example', '--lib', '--doc'])
) {
  const sequential = path.join(__dirname, 'run-rust-tests-sequential.cjs');
  const forwarded = cargoArgs.slice(1).filter((arg) => arg !== '--tests');
  console.error(
    '[run-rust-command] Refusing multi-target cargo test link; ' +
      'delegating to run-rust-tests-sequential.cjs (one binary at a time).',
  );
  const result = spawnSync(process.execPath, [sequential, ...forwarded], {
    cwd: path.join(__dirname, '..'),
    env,
    stdio: 'inherit',
    shell: process.platform === 'win32' ? true : undefined,
  });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

// If a specific test target is specified, strip the broad `--tests` flag so
// Cargo compiles only that single binary instead of all 40+ integration tests.
if (isCargoTest && hasFlag(cargoArgs, ['--test', '--bin', '--example'])) {
  const testsIdx = cargoArgs.indexOf('--tests');
  if (testsIdx !== -1) {
    cargoArgs.splice(testsIdx, 1);
  }
}

if (!env.CARGO_BUILD_JOBS && !hasFlag(cargoArgs, ['-j', '--jobs'])) {
  env.CARGO_BUILD_JOBS = String(getDefaultBuildJobs(cargoArgs));
}

if (
  isCargoTest &&
  !env.RUST_TEST_THREADS &&
  !hasFlag(cargoArgs, ['--test-threads'])
) {
  const testThreads = String(getDefaultTestThreads());
  const separatorIndex = cargoArgs.indexOf('--');

  if (separatorIndex === -1) {
    cargoArgs.push('--', `--test-threads=${testThreads}`);
  } else {
    cargoArgs.splice(separatorIndex + 1, 0, `--test-threads=${testThreads}`);
  }
}

if (isCargoTest && !env.CARGO_INCREMENTAL) {
  // Integration tests compile many near-duplicate binaries; disabling incremental
  // avoids massive dep-graph churn and keeps disk usage from exploding.
  env.CARGO_INCREMENTAL = '0';
}

function buildSpawnTarget(args) {
  if (
    process.platform === 'linux' &&
    isCargoTest &&
    env.LIBRAGENT_NO_NICE !== '1' &&
    commandExists('nice')
  ) {
    // Keep rustc/linker below the desktop shell so Cursor stays responsive.
    if (commandExists('ionice')) {
      return {
        command: 'nice',
        args: ['-n', '10', 'ionice', '-c2', '-n7', 'cargo', ...args],
      };
    }

    return {
      command: 'nice',
      args: ['-n', '10', 'cargo', ...args],
    };
  }

  return { command: 'cargo', args };
}

function resolveStdio() {
  const logPath = env.LIBRAGENT_RUST_TEST_LOG;
  if (logPath) {
    fs.mkdirSync(path.dirname(logPath), { recursive: true });
    const logFd = fs.openSync(logPath, 'a');
    return { stdio: ['ignore', logFd, logFd], logFd };
  }

  return { stdio: 'inherit', logFd: null };
}

const { command, args: spawnArgs } = buildSpawnTarget(cargoArgs);
const { stdio, logFd } = resolveStdio();

if (isCargoTest && stdio === 'inherit') {
  const jobs = env.CARGO_BUILD_JOBS ?? 'default';
  console.error(
    `[run-rust-command] cargo test (jobs=${jobs}, test-threads=1)`,
  );
}

const result = spawnSync(command, spawnArgs, {
  cwd: path.join(__dirname, '..', 'src-tauri'),
  env,
  stdio,
  shell: process.platform === 'win32' ? true : undefined,
});

if (logFd !== null) {
  fs.closeSync(logFd);
}

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);

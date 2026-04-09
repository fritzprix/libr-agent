#!/usr/bin/env node

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
  const cpuCount = Math.max(1, getCpuCount());
  const totalMemGiB = os.totalmem() / 1024 ** 3;
  const cpuLimitedJobs = clamp(cpuCount - 1, 1, 8);
  const memoryLimitedJobs = clamp(Math.floor(totalMemGiB / 3), 1, 8);

  return Math.min(cpuLimitedJobs, memoryLimitedJobs);
}

function getRecommendedTestThreads() {
  const cpuCount = Math.max(1, getCpuCount());
  const totalMemGiB = os.totalmem() / 1024 ** 3;
  const cpuLimitedThreads = clamp(Math.floor(cpuCount / 2), 1, 4);
  const memoryLimitedThreads = clamp(Math.floor(totalMemGiB / 4), 1, 4);

  return Math.min(cpuLimitedThreads, memoryLimitedThreads);
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

const env = { ...process.env };

if (!env.CARGO_BUILD_JOBS && !hasFlag(cargoArgs, ['-j', '--jobs'])) {
  env.CARGO_BUILD_JOBS = String(getRecommendedBuildJobs());
}

const isCargoTest = cargoArgs[0] === 'test';
if (
  isCargoTest &&
  !env.RUST_TEST_THREADS &&
  !hasFlag(cargoArgs, ['--test-threads'])
) {
  const testThreads = String(getRecommendedTestThreads());
  const separatorIndex = cargoArgs.indexOf('--');

  if (separatorIndex === -1) {
    cargoArgs.push('--', `--test-threads=${testThreads}`);
  } else {
    cargoArgs.splice(separatorIndex + 1, 0, `--test-threads=${testThreads}`);
  }
}

const result = spawnSync('cargo', cargoArgs, {
  cwd: path.join(__dirname, '..', 'src-tauri'),
  env,
  stdio: 'inherit',
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);

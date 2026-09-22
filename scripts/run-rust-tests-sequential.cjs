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

function runCargoTest(cargoArgs) {
  const env = { ...process.env };
  if (!env.CARGO_BUILD_JOBS) {
    env.CARGO_BUILD_JOBS = '1';
  }
  const result = spawnSync(process.execPath, [RUNNER, 'test', ...cargoArgs], {
    cwd: ROOT,
    stdio: 'inherit',
    env,
  });
  if (result.error) {
    console.error(result.error.message);
    return 1;
  }
  return result.status ?? 1;
}

function main() {
  const passthrough = process.argv.slice(2).filter((arg) => arg !== '--tests');

  // Single-target / explicit selection: do not fan out.
  if (hasFlag(passthrough, ['--test', '--bin', '--example', '--lib', '--doc'])) {
    process.exit(runCargoTest(passthrough));
  }

  const targets = listIntegrationTestTargets().filter(
    (name) => !shouldSkipTarget(name),
  );

  if (targets.length === 0) {
    console.error('[rust-test-sequential] No integration test targets found.');
    process.exit(1);
  }

  console.error(
    `[rust-test-sequential] ${targets.length} targets — linking one binary at a time (avoids multi-target OOM)`,
  );

  const failed = [];
  for (let i = 0; i < targets.length; i += 1) {
    const target = targets[i];
    console.error(
      `\n[rust-test-sequential] (${i + 1}/${targets.length}) --test ${target}`,
    );
    const status = runCargoTest(['--test', target, ...passthrough]);
    if (status !== 0) {
      failed.push(target);
      console.error(`[rust-test-sequential] FAILED: ${target} (exit ${status})`);
      console.error(
        `[rust-test-sequential] Stopping after first failure. Passed ${i}/${targets.length} before fail.`,
      );
      process.exit(status);
    }
  }

  console.error(
    `\n[rust-test-sequential] All ${targets.length} targets passed.`,
  );
  process.exit(0);
}

main();

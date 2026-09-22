#!/usr/bin/env node
/**
 * Move root-level `tests/*.rs` crates into `tests/integration/` and register
 * them on the consolidated `integration_tests` binary.
 *
 * IMPORTANT:
 * - Merges into existing `integration/mod.rs` (does not wipe prior modules).
 * - Preserves `#![cfg(not(windows))]` on `integration_tests.rs`.
 * - Do NOT use this as the only OOM fix: Windows cannot run the monolith.
 *   Prefer `pnpm rust:test` (sequential per-target linking).
 *
 * Usage: node scripts/consolidate-tests.cjs
 */

const fs = require('node:fs');
const path = require('node:path');

const TESTS_DIR = path.join(__dirname, '..', 'src-tauri', 'tests');
const INTEGRATION_DIR = path.join(TESTS_DIR, 'integration');
const COMMON_DIR = path.join(TESTS_DIR, 'common');

function readExistingModules(modRsPath) {
  if (!fs.existsSync(modRsPath)) {
    return new Set();
  }
  const content = fs.readFileSync(modRsPath, 'utf8');
  const modules = new Set();
  for (const match of content.matchAll(/^\s*pub\s+mod\s+([A-Za-z0-9_]+)\s*;/gm)) {
    modules.add(match[1]);
  }
  return modules;
}

function writeModRs(modRsPath, modules) {
  const sorted = [...modules].sort();
  let content =
    '// Auto-generated module declarations for LibrAgent integration tests\n\n';
  for (const mod of sorted) {
    content += `pub mod ${mod};\n`;
  }
  fs.writeFileSync(modRsPath, content, 'utf8');
}

function ensureIntegrationTestsEntrypoint() {
  const integrationTestsRsPath = path.join(TESTS_DIR, 'integration_tests.rs');
  const expected = `#![cfg(not(windows))]

// This consolidated integration binary links the full Tauri/WebView path and
// crashes before the test harness starts on Windows (STATUS_ENTRYPOINT_NOT_FOUND).
// Keep Linux/macOS coverage here until the test layout is split into Windows-safe targets.

// LibrAgent Consolidated Integration Tests
mod common;
mod integration;
`;
  if (!fs.existsSync(integrationTestsRsPath)) {
    fs.writeFileSync(integrationTestsRsPath, expected, 'utf8');
    console.log(`Created: ${integrationTestsRsPath}`);
    return;
  }
  const current = fs.readFileSync(integrationTestsRsPath, 'utf8');
  if (!current.includes('#!\[cfg(not(windows))\]') && !current.includes('#![cfg(not(windows))]')) {
    console.warn(
      'WARNING: integration_tests.rs is missing #![cfg(not(windows))] — not overwriting. Fix manually.',
    );
    return;
  }
  console.log(`Kept existing entrypoint: ${integrationTestsRsPath}`);
}

function main() {
  console.log('Starting test consolidation...');
  console.log(`Tests directory: ${TESTS_DIR}`);
  console.log(`Integration directory: ${INTEGRATION_DIR}`);

  if (!fs.existsSync(INTEGRATION_DIR)) {
    fs.mkdirSync(INTEGRATION_DIR, { recursive: true });
  }
  if (!fs.existsSync(COMMON_DIR)) {
    fs.mkdirSync(COMMON_DIR, { recursive: true });
  }

  const commonRsPath = path.join(TESTS_DIR, 'common.rs');
  const commonModPath = path.join(COMMON_DIR, 'mod.rs');
  if (fs.existsSync(commonRsPath)) {
    fs.renameSync(commonRsPath, commonModPath);
    console.log('Moved common.rs to common/mod.rs');
  } else if (!fs.existsSync(commonModPath)) {
    console.error('ERROR: Neither common.rs nor common/mod.rs exists!');
    process.exit(1);
  }

  const modRsPath = path.join(INTEGRATION_DIR, 'mod.rs');
  const modules = readExistingModules(modRsPath);

  const rustTestFiles = fs
    .readdirSync(TESTS_DIR)
    .filter((file) => file.endsWith('.rs'))
    .filter((file) => fs.statSync(path.join(TESTS_DIR, file)).isFile())
    .filter((file) => file !== 'common.rs' && file !== 'integration_tests.rs');

  console.log(`Found ${rustTestFiles.length} root-level Rust test files to consolidate.`);

  let moved = 0;
  for (const file of rustTestFiles) {
    const moduleName = file.slice(0, -3);
    const oldPath = path.join(TESTS_DIR, file);
    const newPath = path.join(INTEGRATION_DIR, file);

    if (fs.existsSync(newPath)) {
      console.error(
        `ERROR: Refusing to overwrite existing integration/${file}. Resolve duplicate first.`,
      );
      process.exit(1);
    }

    let content = fs.readFileSync(oldPath, 'utf8');
    content = content.replace(
      /^\s*(pub\s+)?(pub\(crate\)\s+)?mod\s+common\s*;/gm,
      'use crate::common;',
    );
    content = content.replace(
      /#\[path\s*=\s*"\.\.\/build_support\//g,
      '#[path = "../../build_support/',
    );

    fs.writeFileSync(newPath, content, 'utf8');
    fs.unlinkSync(oldPath);
    modules.add(moduleName);
    moved += 1;
    console.log(`Consolidated: ${file} -> integration/${file}`);
  }

  writeModRs(modRsPath, modules);
  console.log(`Updated: ${modRsPath} (${modules.size} modules)`);
  ensureIntegrationTestsEntrypoint();

  console.log(
    `Test consolidation complete (moved ${moved}). Prefer pnpm rust:test for sequential linking.`,
  );
}

main();

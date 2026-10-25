#!/usr/bin/env node
/**
 * Verifies that the `libragent` package version in `src-tauri/Cargo.lock`
 * matches the `version` declared in `src-tauri/Cargo.toml`.
 */

const fs = require('fs');
const path = require('path');

const rootDir = path.resolve(__dirname, '..');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const cargoLockPath = path.join(rootDir, 'src-tauri', 'Cargo.lock');

if (!fs.existsSync(cargoTomlPath)) {
  console.error(`Error: Cargo.toml not found at ${cargoTomlPath}`);
  process.exit(1);
}

if (!fs.existsSync(cargoLockPath)) {
  console.error(`Error: Cargo.lock not found at ${cargoLockPath}`);
  process.exit(1);
}

const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
const tomlVersionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
if (!tomlVersionMatch) {
  console.error('Error: Could not extract version from Cargo.toml');
  process.exit(1);
}
const tomlVersion = tomlVersionMatch[1];

const cargoLock = fs.readFileSync(cargoLockPath, 'utf8');
const lockVersionMatch = cargoLock.match(
  /\[\[package\]\]\s*\nname\s*=\s*"libragent"\s*\nversion\s*=\s*"([^"]+)"/m,
);
if (!lockVersionMatch) {
  console.error(
    'Error: Could not extract libragent package version from Cargo.lock',
  );
  process.exit(1);
}
const lockVersion = lockVersionMatch[1];

if (tomlVersion !== lockVersion) {
  console.error(
    `❌ Version mismatch: Cargo.toml is v${tomlVersion}, but Cargo.lock has libragent v${lockVersion}.\n` +
      `Fix this by running: (cd src-tauri && cargo update -p libragent --offline)`,
  );
  process.exit(1);
}

console.log(
  `✅ [OK] Cargo.lock libragent version (v${lockVersion}) matches Cargo.toml (v${tomlVersion})`,
);

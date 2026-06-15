#!/usr/bin/env node

const { execSync } = require('node:child_process');
const { readFileSync } = require('node:fs');
const path = require('node:path');

const userAgent = process.env.npm_config_user_agent ?? '';
if (!userAgent.includes('pnpm')) {
  process.exit(0);
}

const packageJsonPath = path.join(__dirname, '..', 'package.json');
const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
const packageManager = packageJson.packageManager ?? '';

const match = packageManager.match(/^pnpm@(\d+\.\d+\.\d+)$/);
if (!match) {
  console.warn(
    '[enforce-pnpm-version] package.json is missing a pinned packageManager field (pnpm@x.y.z).',
  );
  process.exit(0);
}

const expectedVersion = match[1];
let actualVersion = '';

try {
  actualVersion = execSync('pnpm --version', {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  }).trim();
} catch {
  console.error(
    `[enforce-pnpm-version] Unable to read pnpm version. Expected pnpm@${expectedVersion}.`,
  );
  process.exit(1);
}

if (actualVersion === expectedVersion) {
  process.exit(0);
}

console.error(
  [
    `[enforce-pnpm-version] Expected pnpm@${expectedVersion} (packageManager), but found pnpm@${actualVersion}.`,
    'Using a different pnpm major/minor can rewrite pnpm-lock.yaml and break CI frozen installs.',
    '',
    'Fix:',
    '  corepack enable',
    `  corepack prepare pnpm@${expectedVersion} --activate`,
    '  pnpm install --frozen-lockfile',
  ].join('\n'),
);
process.exit(1);

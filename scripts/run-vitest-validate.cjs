#!/usr/bin/env node

const { spawnSync } = require('node:child_process');

const pnpmCommand = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
const extraArgs = process.argv.slice(2);

const env = {
  ...process.env,
  LIBRAGENT_LOW_MEMORY_VALIDATE:
    process.env.LIBRAGENT_LOW_MEMORY_VALIDATE ?? '1',
};

const result = spawnSync(pnpmCommand, ['vitest', 'run', ...extraArgs], {
  env,
  stdio: 'inherit',
  shell: process.platform === 'win32' ? true : undefined,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
